use crate::commands::Output;
use crate::commands::aws::utils::aws_datetime_to_local;
use crate::commands::aws::{
    arg::GlobalOptions,
    ec2::arg::{AWSEC2Command, EC2SubCommand, SearchArg, TerminateArg},
    error::{
        handle_construction_failure, handle_dispatch_failure, handle_response_error,
        handle_service_error, handle_timeout_error, handle_unknown_error,
    },
};
use aws_sdk_ec2::{Client, types::Instance};
use dialoguer::Confirm;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::Write;
use tokio::time::{Duration, Instant, interval};

impl AWSEC2Command {
    pub async fn execute(self, opts: GlobalOptions) -> Result<Output, Box<dyn std::error::Error>> {
        let client = Client::new(&opts.sdk_config);
        match self.command {
            EC2SubCommand::Get(args) => args.execute(&client, opts).await,
            EC2SubCommand::Terminate(args) => args.execute(&client, opts).await,
        }
    }
}

impl SearchArg {
    async fn execute(
        self,
        client: &Client,
        opts: GlobalOptions,
    ) -> Result<Output, Box<dyn std::error::Error>> {
        // Validate search parameters
        if let Err(e) = self.validate() {
            return Err(format!("Error: {}", e).into());
        }

        let output = Output::new(opts.verbose);
        if opts.verbose {
            output.stderr("Searching for EC2 instances...");
        }

        // Get instances with filters applied
        match client
            .describe_instances()
            .set_filters(self.build_filters())
            .send()
            .await
        {
            Ok(response) => {
                let mut instances = Vec::new();

                for reservation in response.reservations() {
                    for instance in reservation.instances() {
                        // Check if instance matches our criteria
                        if self.matches_instance(instance) {
                            if opts.verbose {
                                instances.push(instance.long());
                            } else {
                                instances.push(instance.short());
                            }
                        }
                    }
                }

                output.stdout(&serde_json::to_string_pretty(&instances).unwrap());
                Ok(output)
            }
            Err(err) => {
                use aws_sdk_ec2::error::SdkError;

                let error_msg = match err {
                    SdkError::ServiceError(service_err) => {
                        handle_service_error(service_err.err(), "search EC2 instances")
                    }
                    SdkError::DispatchFailure(dispatch_err) => {
                        handle_dispatch_failure(&dispatch_err, "search EC2 instances")
                    }
                    SdkError::ConstructionFailure(err) => {
                        handle_construction_failure(&err, "search EC2 instances", opts.verbose)
                    }
                    SdkError::TimeoutError(err) => {
                        handle_timeout_error(&err, "search EC2 instances", opts.verbose)
                    }
                    SdkError::ResponseError(err) => {
                        handle_response_error(&err, "search EC2 instances", opts.verbose)
                    }
                    _ => handle_unknown_error(&err, "search EC2 instances", opts.verbose),
                };

                Err(error_msg.into())
            }
        }
    }

    fn build_filters(&self) -> Option<Vec<aws_sdk_ec2::types::Filter>> {
        use aws_sdk_ec2::types::Filter;
        let mut filters = Vec::new();

        // Filter by exact name match (only when not using fuzzy matching)
        // Note: AWS filters don't support fuzzy matching, so when --fuzzy is used,
        // we skip the name filter and do fuzzy matching client-side
        if let Some(name) = &self.name {
            if !self.fuzzy {
                filters.push(Filter::builder().name("tag:Name").values(name).build());
            }
        }

        // Filter by private IP
        if let Some(private_ip) = &self.private_ip {
            filters.push(
                Filter::builder()
                    .name("private-ip-address")
                    .values(private_ip)
                    .build(),
            );
        }

        // Filter by public IP
        if let Some(public_ip) = &self.public_ip {
            filters.push(
                Filter::builder()
                    .name("ip-address")
                    .values(public_ip)
                    .build(),
            );
        }

        // Filter by instance ID
        if let Some(id) = &self.instance_id {
            filters.push(Filter::builder().name("instance-id").values(id).build())
        }

        // Filter by instance state
        if !self.state.is_empty() {
            // Use the specified states
            let mut state_filter = Filter::builder().name("instance-state-name");
            for state in &self.state {
                state_filter = state_filter.values(state);
            }
            filters.push(state_filter.build());
        } else {
            // Default: filter out terminated instances for better performance
            filters.push(
                Filter::builder()
                    .name("instance-state-name")
                    .values("running")
                    .values("pending")
                    .values("stopping")
                    .values("stopped")
                    .build(),
            );
        }

        if filters.is_empty() {
            return None;
        }
        Some(filters)
    }

    fn matches_instance(&self, instance: &Instance) -> bool {
        // Check fuzzy name matches
        if let Some(name) = &self.name {
            if let Some(instance_name) = instance.get_name() {
                if self.fuzzy {
                    return instance_name.to_lowercase().contains(&name.to_lowercase());
                }
                return instance_name == name;
            }
            return false;
        }

        // For exact matches and IP matches, AWS filters have already done the work
        true
    }
}

trait InstanceExt {
    fn get_name(&self) -> Option<&str>;
    fn get_tags(&self) -> HashMap<&str, &str>;
    fn get_console_url(&self) -> Option<String>;
    fn short(&self) -> Value;
    fn long(&self) -> Value;
}

impl InstanceExt for Instance {
    fn get_name(&self) -> Option<&str> {
        let tags = self.tags();
        for tag in tags {
            if tag.key() == Some("Name") {
                return tag.value();
            }
        }
        None
    }

    fn get_tags(&self) -> HashMap<&str, &str> {
        let mut tag_map = HashMap::new();
        let tags = self.tags();
        for tag in tags {
            if let (Some(key), Some(value)) = (tag.key(), tag.value()) {
                tag_map.insert(key, value);
            }
        }
        tag_map
    }

    fn short(&self) -> Value {
        let mut instance_json = json!({
            "InstanceId": self.instance_id().unwrap_or("N/A"),
            "Name": self.get_name().unwrap_or("N/A"),
            "State": self.state().and_then(|s| s.name()).map(|n| n.as_str()).unwrap_or("unknown"),
            "InstanceType": self.instance_type().map(|t| t.as_str()).unwrap_or("unknown"),
            "PrivateIpAddress": self.private_ip_address().unwrap_or("N/A"),
            "PublicIpAddress": self.public_ip_address().unwrap_or("N/A"),
        });

        // Add additional details for long format
        if let Some(launch_time) = self.launch_time() {
            instance_json["LaunchTime"] = json!(aws_datetime_to_local(launch_time).to_string());
        }

        // Add ASG information
        let tags = self.get_tags();
        for (key, value) in tags {
            if key == "aws:autoscaling:groupName" {
                instance_json["AutoScalingGroup"] = json!(value);
                break;
            }
        }

        // Add console URL
        if let Some(url) = self.get_console_url() {
            instance_json["_url"] = json!(url);
        }

        instance_json
    }

    fn long(&self) -> Value {
        let mut instance_json = self.short();

        // Add all tags
        let tags = self.get_tags();
        if !tags.is_empty() {
            instance_json["Tags"] = json!(tags);
        }

        // Add VPC ID
        if let Some(vpc_id) = self.vpc_id() {
            instance_json["VpcId"] = json!(vpc_id);
        }

        // Add Subnet ID
        if let Some(subnet_id) = self.subnet_id() {
            instance_json["SubnetId"] = json!(subnet_id);
        }

        if let Some(placement) = self.placement() {
            if let Some(availability_zone) = placement.availability_zone() {
                instance_json["AvailabilityZone"] = json!(availability_zone);
            }
        }

        if let Some(vpc_id) = self.vpc_id() {
            instance_json["VpcId"] = json!(vpc_id);
        }

        if let Some(subnet_id) = self.subnet_id() {
            instance_json["SubnetId"] = json!(subnet_id);
        }

        instance_json
    }

    fn get_console_url(&self) -> Option<String> {
        self.instance_id().map(|instance_id| {
            format!(
                "https://console.aws.amazon.com/ec2/home#InstanceDetails:instanceId={}",
                instance_id
            )
        })
    }
}

impl TerminateArg {
    async fn execute(
        self,
        client: &Client,
        opts: GlobalOptions,
    ) -> Result<Output, Box<dyn std::error::Error>> {
        let output = Output::new(opts.verbose);

        // Fetch instance details
        output.stderr(&format!(
            "Fetching instance details for {}...",
            self.instance_id
        ));
        let instance = match get_instance_by_id(client, &self.instance_id).await {
            Ok(Some(inst)) => inst,
            Ok(None) => {
                return Err(format!("Instance {} not found", self.instance_id).into());
            }
            Err(e) => {
                return Err(format!("Error fetching instance: {}", e).into());
            }
        };

        // Show instance details
        output.stderr("\n=== Instance Details ===");
        if opts.verbose {
            output.stdout(&serde_json::to_string_pretty(&instance.long()).unwrap());
        } else {
            output.stdout(&serde_json::to_string_pretty(&instance.short()).unwrap());
        }

        // Check if instance is already terminated or terminating
        let current_state = instance
            .state()
            .and_then(|s| s.name())
            .map(|n| n.as_str())
            .unwrap_or("unknown");

        if current_state == "terminated" {
            return Err(format!("Instance {} is already terminated", self.instance_id).into());
        }

        if current_state == "shutting-down" {
            return Err(format!("Instance {} is already shutting down", self.instance_id).into());
        }

        // Note: Termination protection check will be caught during actual termination
        // if enabled, AWS will return an error that we'll handle gracefully

        // Check if instance is in an Auto Scaling Group
        let tags = instance.get_tags();
        let is_in_asg = tags.contains_key("aws:autoscaling:groupName");
        if is_in_asg {
            output.stderr("\n⚠️  WARNING: This instance is part of an Auto Scaling Group.");
            output.stderr("   Consider using ASG detach/terminate operations instead.");
        }

        // Confirmation prompt (unless --yes flag is used)
        if !self.yes {
            let prompt = format!(
                "\n⚠️  WARNING: You are about to terminate instance {}. This action cannot be undone!\n   Continue?",
                self.instance_id
            );
            if !Confirm::new()
                .with_prompt(prompt)
                .interact()
                .unwrap_or(false)
            {
                output.stderr("Termination cancelled.");
                return Ok(output);
            }
        }

        // Step 1: Stop the instance (if not already stopped)
        if current_state != "stopped" && current_state != "stopping" {
            output.stderr(&format!(
                "\n[Step 1/2] Stopping instance {}...",
                self.instance_id
            ));

            match client
                .stop_instances()
                .instance_ids(&self.instance_id)
                .send()
                .await
            {
                Ok(_) => {
                    output.stderr("✓ Stop request sent successfully");
                }
                Err(err) => {
                    use aws_sdk_ec2::error::SdkError;
                    let error_msg = match err {
                        SdkError::ServiceError(service_err) => {
                            handle_service_error(service_err.err(), "stop EC2 instance")
                        }
                        SdkError::DispatchFailure(dispatch_err) => {
                            handle_dispatch_failure(&dispatch_err, "stop EC2 instance")
                        }
                        SdkError::ConstructionFailure(err) => {
                            handle_construction_failure(&err, "stop EC2 instance", opts.verbose)
                        }
                        SdkError::TimeoutError(err) => {
                            handle_timeout_error(&err, "stop EC2 instance", opts.verbose)
                        }
                        SdkError::ResponseError(err) => {
                            handle_response_error(&err, "stop EC2 instance", opts.verbose)
                        }
                        _ => handle_unknown_error(&err, "stop EC2 instance", opts.verbose),
                    };
                    return Err(error_msg.into());
                }
            }

            // Poll until stopped
            output.stderr("Waiting for instance to stop...");
            match poll_instance_state(client, &self.instance_id, "stopped", &output, opts.verbose)
                .await
            {
                Ok(_) => {
                    output.stderr("✓ Instance stopped successfully");
                }
                Err(e) => {
                    return Err(format!("Failed to stop instance: {}", e).into());
                }
            }
        } else if current_state == "stopping" {
            output.stderr(&format!(
                "\n[Step 1/2] Instance is already stopping, waiting for stop to complete..."
            ));
            match poll_instance_state(client, &self.instance_id, "stopped", &output, opts.verbose)
                .await
            {
                Ok(_) => {
                    output.stderr("✓ Instance stopped successfully");
                }
                Err(e) => {
                    return Err(format!("Failed to stop instance: {}", e).into());
                }
            }
        } else {
            output.stderr(&format!(
                "\n[Step 1/2] Instance is already stopped, skipping stop step"
            ));
        }

        // Step 2: Terminate the instance
        output.stderr(&format!(
            "\n[Step 2/2] Terminating instance {}...",
            self.instance_id
        ));

        match client
            .terminate_instances()
            .instance_ids(&self.instance_id)
            .send()
            .await
        {
            Ok(_) => {
                output.stderr("✓ Termination request sent successfully");
            }
            Err(err) => {
                use aws_sdk_ec2::error::SdkError;
                let error_msg = match err {
                    SdkError::ServiceError(service_err) => {
                        handle_service_error(service_err.err(), "terminate EC2 instance")
                    }
                    SdkError::DispatchFailure(dispatch_err) => {
                        handle_dispatch_failure(&dispatch_err, "terminate EC2 instance")
                    }
                    SdkError::ConstructionFailure(err) => {
                        handle_construction_failure(&err, "terminate EC2 instance", opts.verbose)
                    }
                    SdkError::TimeoutError(err) => {
                        handle_timeout_error(&err, "terminate EC2 instance", opts.verbose)
                    }
                    SdkError::ResponseError(err) => {
                        handle_response_error(&err, "terminate EC2 instance", opts.verbose)
                    }
                    _ => handle_unknown_error(&err, "terminate EC2 instance", opts.verbose),
                };
                return Err(error_msg.into());
            }
        }

        // Poll until terminated
        output.stderr("Waiting for instance to terminate...");
        match poll_instance_state(
            client,
            &self.instance_id,
            "terminated",
            &output,
            opts.verbose,
        )
        .await
        {
            Ok(_) => {
                output.stderr("✓ Instance terminated successfully");
            }
            Err(e) => {
                return Err(format!("Failed to verify termination: {}", e).into());
            }
        }

        output.stderr(&format!(
            "\n✅ Instance {} has been successfully terminated.",
            self.instance_id
        ));

        Ok(output)
    }
}

// Helper function to get an instance by ID
async fn get_instance_by_id(
    client: &Client,
    instance_id: &str,
) -> Result<Option<Instance>, Box<dyn std::error::Error>> {
    match client
        .describe_instances()
        .instance_ids(instance_id)
        .send()
        .await
    {
        Ok(response) => {
            for reservation in response.reservations() {
                for instance in reservation.instances() {
                    if instance
                        .instance_id()
                        .map(|id| id == instance_id)
                        .unwrap_or(false)
                    {
                        return Ok(Some(instance.clone()));
                    }
                }
            }
            Ok(None)
        }
        Err(err) => Err(format!("Failed to describe instance: {}", err).into()),
    }
}

// Helper function to poll instance state with progress updates
// Uses tokio::select! with intervals for clean concurrent polling and rendering
async fn poll_instance_state(
    client: &Client,
    instance_id: &str,
    target_state: &str,
    _output: &Output,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    const MAX_ATTEMPTS: u32 = 60; // Maximum number of polling attempts (5 minutes at 5s intervals)
    const POLL_INTERVAL: Duration = Duration::from_secs(5);
    const RENDER_INTERVAL: Duration = Duration::from_millis(150);

    let client = client.clone();
    let instance_id = instance_id.to_string();
    let target_state = target_state.to_string();

    // Create intervals for polling and rendering
    let mut poll_interval = interval(POLL_INTERVAL);
    let mut render_interval = interval(RENDER_INTERVAL);

    // Skip the first tick of both intervals to ensure the full duration is waited
    poll_interval.tick().await;
    render_interval.tick().await;

    // State variables
    let start_time = Instant::now();
    let mut attempt = 0;
    let mut frame = 0;
    let mut current_state: Option<String> = None;
    let mut is_complete = false;
    let mut final_render_done = false;
    let mut error: Option<String> = None;

    // Main polling and rendering loop
    loop {
        tokio::select! {
            _ = poll_interval.tick() => {
                attempt += 1;

                if attempt > MAX_ATTEMPTS {
                    error = Some(format!(
                        "Timeout: Instance did not reach '{}' state within {} attempts",
                        target_state, MAX_ATTEMPTS
                    ));
                    is_complete = true;
                    // Will break in render branch after showing final state
                } else {
                    // Check instance state - await and immediately extract to owned values
                    let (action, state_update) = {
                        let instance_result = get_instance_by_id(&client, &instance_id).await;
                        match instance_result {
                            Ok(Some(instance)) => {
                                let state_str = instance
                                    .state()
                                    .and_then(|s| s.name())
                                    .map(|n| n.as_str())
                                    .unwrap_or("unknown")
                                    .to_string();
                                drop(instance);
                                (None, Some(state_str))
                            }
                            Ok(None) => {
                                if target_state == "terminated" {
                                    (Some("complete"), None)
                                } else {
                                    (Some("error_not_found"), None)
                                }
                            }
                            Err(_e) => {
                                (None, None) // Continue polling
                            }
                        }
                    };

                    // Handle actions
                    if let Some(action) = action {
                        match action {
                            "complete" => {
                                is_complete = true;
                            }
                            "error_not_found" => {
                                error = Some("Instance not found".to_string());
                                is_complete = true;
                            }
                            _ => {}
                        }
                    }

                    // Update state if we got one
                    if let Some(state_str) = state_update {
                        current_state = Some(state_str.clone());

                        if state_str == target_state {
                            is_complete = true;
                        }

                        // Check for error states
                        if target_state == "stopped" && state_str == "terminated" {
                            error = Some("Instance was terminated before it could be stopped".to_string());
                            is_complete = true;
                        }
                    }
                }
            }

            _ = render_interval.tick() => {
                if is_complete && !final_render_done {
                    // Final render - clear the line completely and print on new line
                    // Use spaces to clear any leftover characters from the progress line
                    eprint!("\r{}", " ".repeat(80)); // Clear the entire line
                    eprint!("\r"); // Move back to start

                    if let Some(ref err) = error {
                        eprintln!("  Error: {}", err);
                    } else {
                        let elapsed = start_time.elapsed().as_secs();
                        let current = current_state.as_deref().unwrap_or("unknown");
                        if current == target_state || target_state == "terminated" {
                            eprintln!(
                                "  State: {} (reached target in {}s)",
                                target_state, elapsed
                            );
                        } else {
                            eprintln!(
                                "  State: {} (completed in {}s)",
                                target_state, elapsed
                            );
                        }
                    }
                    let _ = std::io::stderr().flush();
                    final_render_done = true;
                } else if !is_complete {
                    // Animate progress
                    frame += 1;
                    let elapsed = start_time.elapsed().as_secs();

                    if verbose {
                        let current = current_state.as_deref().unwrap_or("checking");
                        eprint!(
                            "\r  {} Waiting for '{}' state (current: '{}') - {}s elapsed  ",
                            get_progress_char(frame),
                            target_state,
                            current,
                            elapsed
                        );
                    } else {
                        eprint!(
                            "\r  {} Waiting for '{}' state ({}s elapsed)  ",
                            get_progress_char(frame),
                            target_state,
                            elapsed
                        );
                    }
                    let _ = std::io::stderr().flush();
                }
            }
        }

        // Break out of loop when complete and final render has been shown
        if is_complete && final_render_done {
            break;
        }
    }

    // Check if there was an error
    if let Some(err) = error {
        return Err(err.into());
    }

    Ok(())
}

// Helper function to get a progress character
fn get_progress_char(attempt: usize) -> &'static str {
    match attempt % 4 {
        0 => "|",
        1 => "/",
        2 => "-",
        3 => "\\",
        _ => "|",
    }
}
