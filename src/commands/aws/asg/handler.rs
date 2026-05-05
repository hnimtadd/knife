use dialoguer::Confirm;
use serde_json::{Value, json};
use std::collections::HashMap;

use crate::commands::Output;
use crate::commands::aws::asg::arg::AttachInstancesArg;
use crate::commands::aws::{
    arg::GlobalOptions,
    asg::arg::{ASGSubCommand, AWSASGCommand, DetachInstancesArg, ScaleArg},
    error::{
        handle_construction_failure, handle_dispatch_failure, handle_response_error,
        handle_service_error, handle_timeout_error, handle_unknown_error,
    },
};
use aws_sdk_autoscaling::{Client, types::AutoScalingGroup};

impl AWSASGCommand {
    pub async fn execute(self, opts: GlobalOptions) -> Result<Output, Box<dyn std::error::Error>> {
        let client = Client::new(&opts.sdk_config);
        match self.command {
            ASGSubCommand::Get => self.get_asg(&client, &opts).await,
            ASGSubCommand::Scale(args) => args.execute(self.name, &client, &opts).await,
            ASGSubCommand::DetachInstances(args) => args.execute(self.name, &client, &opts).await,
            ASGSubCommand::AttachInstances(args) => args.execute(self.name, &client, &opts).await,
        }
    }

    async fn get_asg(
        self,
        client: &Client,
        opts: &GlobalOptions,
    ) -> Result<Output, Box<dyn std::error::Error>> {
        match get_asg_details(client, &self.name, opts.verbose).await {
            Ok(asg) => {
                let output = Output::new(opts.verbose);
                output.stdout(&serde_json::to_string_pretty(&asg).unwrap());
                Ok(output)
            }
            Err(error_msg) => Err(format!("Error getting ASG: {}", error_msg).into()),
        }
    }
}

// Reusable function to get ASG details
async fn get_asg_details(client: &Client, asg_name: &str, verbose: bool) -> Result<Value, String> {
    if asg_name.is_empty() {
        return Err("Invalid ASG name.".to_string());
    }

    match client
        .describe_auto_scaling_groups()
        .set_auto_scaling_group_names(Some(vec![asg_name.to_string()]))
        .set_max_records(Some(1))
        .send()
        .await
    {
        Ok(response) => {
            if let Some(asg) = response.auto_scaling_groups().first() {
                let asg_value = if verbose { asg.long() } else { asg.short() };
                Ok(asg_value)
            } else {
                Err("ASG not found.".to_string())
            }
        }
        Err(err) => {
            use aws_sdk_autoscaling::error::SdkError;
            let error_msg = match err {
                SdkError::ServiceError(service_err) => {
                    handle_service_error(service_err.err(), "get ASG details")
                }
                SdkError::DispatchFailure(dispatch_err) => {
                    handle_dispatch_failure(&dispatch_err, "get ASG details")
                }
                SdkError::ConstructionFailure(err) => {
                    handle_construction_failure(&err, "get ASG details", false)
                }
                SdkError::TimeoutError(err) => handle_timeout_error(&err, "get ASG details", false),
                SdkError::ResponseError(err) => {
                    handle_response_error(&err, "get ASG details", false)
                }
                _ => handle_unknown_error(&err, "get ASG details", false),
            };
            Err(error_msg.into())
        }
    }
}

trait AutoScalingGroupExt {
    fn long(&self) -> Value;
    fn short(&self) -> Value;
    fn get_tags(&self) -> HashMap<String, String>;
    fn get_console_url(&self) -> Option<String>;
}
impl AutoScalingGroupExt for AutoScalingGroup {
    fn short(&self) -> Value {
        let mut record_json = json!({
            "Name": self.auto_scaling_group_name().unwrap_or("N/A"),
            "ARN": self.auto_scaling_group_arn().unwrap_or("N/A"),
            "Status": self.status().unwrap_or("N/A"),
            "Capacity": json!({
                "MinSize": self.min_size().unwrap_or(-1),
                "MaxSize": self.max_size().unwrap_or(-1),
                "Desired": self.desired_capacity().unwrap_or(-1),
                "Current": self.instances().len(),
            }),
            "HealthCheckType": self.health_check_type().unwrap_or("N/A"),
            "AvailabilityZones": self.availability_zones(),
        });

        // Add LaunchTemplate info if available
        if let Some(lt) = self.launch_template() {
            record_json["LaunchTemplate"] = json!({
                "Name": lt.launch_template_name().unwrap_or("N/A"),
                "Version": lt.version().unwrap_or("N/A"),
            });
        } else if let Some(lc_name) = self.launch_configuration_name() {
            record_json["LaunchConfiguration"] = json!(lc_name);
        }

        // Add VPC zone identifier
        if let Some(vpc_zone_id) = self.vpc_zone_identifier() {
            if !vpc_zone_id.is_empty() {
                record_json["VPCZoneIdentifier"] = json!(vpc_zone_id);
            }
        }

        // Add instance details
        let instances: Vec<&str> = self
            .instances()
            .iter()
            .map(|inst| inst.instance_id().unwrap_or("N/A"))
            .collect();
        if !instances.is_empty() {
            record_json["Instances"] = json!(instances);
        }

        // Add Target Group ARNs
        let target_groups: Vec<&str> = self
            .target_group_arns()
            .iter()
            .map(|s| s.as_str())
            .collect();
        if !target_groups.is_empty() {
            record_json["TargetGroups"] = json!(target_groups);
        }

        // Add console URL
        if let Some(url) = self.get_console_url() {
            record_json["_url"] = json!(url);
        }

        record_json
    }

    fn long(&self) -> Value {
        let mut record_json = self.short();

        // Add tags as HashMap for consistency with EC2/ELB handlers
        let tags = self.get_tags();
        if !tags.is_empty() {
            record_json["Tags"] = json!(tags);
        }

        // Add created time
        if let Some(created_time) = self.created_time() {
            record_json["CreatedTime"] = json!(created_time.to_string());
        }

        // Add health check configuration
        record_json["HealthCheckGracePeriod"] =
            json!(self.health_check_grace_period().unwrap_or(0));

        // Add default cooldown
        record_json["DefaultCooldown"] = json!(self.default_cooldown().unwrap_or(0));

        // Add load balancer names (classic load balancers)
        let lb_names: Vec<&str> = self
            .load_balancer_names()
            .iter()
            .map(|s| s.as_str())
            .collect();
        if !lb_names.is_empty() {
            record_json["LoadBalancerNames"] = json!(lb_names);
        }

        // Add termination policies
        let termination_policies: Vec<&str> = self
            .termination_policies()
            .iter()
            .map(|s| s.as_str())
            .collect();
        if !termination_policies.is_empty() {
            record_json["TerminationPolicies"] = json!(termination_policies);
        }

        // Add enabled metrics
        let enabled_metrics: Vec<Value> = self
            .enabled_metrics()
            .iter()
            .map(|m| {
                json!({
                    "Metric": m.metric().unwrap_or("N/A"),
                    "Granularity": m.granularity().unwrap_or("N/A"),
                })
            })
            .collect();
        if !enabled_metrics.is_empty() {
            record_json["EnabledMetrics"] = json!(enabled_metrics);
        }

        // Add suspended processes
        let suspended_processes: Vec<Value> = self
            .suspended_processes()
            .iter()
            .map(|sp| {
                json!({
                    "ProcessName": sp.process_name().unwrap_or("N/A"),
                    "SuspensionReason": sp.suspension_reason().unwrap_or("N/A"),
                })
            })
            .collect();
        if !suspended_processes.is_empty() {
            record_json["SuspendedProcesses"] = json!(suspended_processes);
        }

        // Add instance details
        let instances: Vec<Value> = self
            .instances()
            .iter()
            .map(|inst| {
                json!({
                    "InstanceId": inst.instance_id().unwrap_or("N/A"),
                    "InstanceType": inst.instance_type().unwrap_or("N/A"),
                    "AvailabilityZone": inst.availability_zone().unwrap_or("N/A"),
                    "LifecycleState": inst.lifecycle_state().map(|s| s.as_str()).unwrap_or("N/A"),
                    "HealthStatus": inst.health_status().unwrap_or("N/A"),
                    "ProtectedFromScaleIn": inst.protected_from_scale_in().unwrap_or(false),
                })
            })
            .collect();
        if !instances.is_empty() {
            record_json["Instances"] = json!(instances);
        }

        // Add new instances protected from scale in
        if let Some(protected) = self.new_instances_protected_from_scale_in() {
            record_json["NewInstancesProtectedFromScaleIn"] = json!(protected);
        }

        // Add service linked role ARN
        if let Some(role_arn) = self.service_linked_role_arn() {
            record_json["ServiceLinkedRoleARN"] = json!(role_arn);
        }

        // Add max instance lifetime
        if let Some(max_lifetime) = self.max_instance_lifetime() {
            record_json["MaxInstanceLifetime"] = json!(max_lifetime);
        }

        // Add capacity rebalance
        if let Some(capacity_rebalance) = self.capacity_rebalance() {
            record_json["CapacityRebalance"] = json!(capacity_rebalance);
        }

        record_json
    }

    fn get_tags(&self) -> HashMap<String, String> {
        let mut tag_map = HashMap::new();
        for tag in self.tags() {
            if let (Some(key), Some(value)) = (tag.key(), tag.value()) {
                tag_map.insert(key.to_string(), value.to_string());
            }
        }
        tag_map
    }

    fn get_console_url(&self) -> Option<String> {
        self.auto_scaling_group_arn().map(|arn| {
            // ARN format: arn:aws:autoscaling:region:account-id:autoScalingGroup:id:autoScalingGroupName/name
            let parts: Vec<&str> = arn.split(':').collect();
            if parts.len() >= 4 {
                let region = parts[3];
                if let Some(name) = self.auto_scaling_group_name() {
                    return format!(
                        "https://{}.console.aws.amazon.com/ec2/home?region={}#AutoScalingGroupDetails:id={};view=details",
                        region, region, urlencoding::encode(name)
                    );
                }
            }
            format!("https://console.aws.amazon.com/ec2/home#AutoScalingGroups:")
        })
    }
}

impl ScaleArg {
    async fn execute(
        self,
        name: String,
        client: &Client,
        opts: &GlobalOptions,
    ) -> Result<Output, Box<dyn std::error::Error>> {
        if name.is_empty() {
            return Err(format!("Error: Invalid ASG name.").into());
        }

        // Validate min/max size relationship
        if let (Some(min), Some(max)) = (self.min_size, self.max_size) {
            if min > max {
                return Err(format!(
                    "Error: Min size ({}) cannot be greater than max size ({}).",
                    min, max
                )
                .into());
            }
        }

        let output = Output::new(opts.verbose);
        // Show ASG state before scaling
        output.stderr("ASG State BEFORE Scaling:");
        match get_asg_details(client, &name, opts.verbose).await {
            Ok(asg) => {
                output.stdout(&serde_json::to_string_pretty(&asg).unwrap());
            }
            Err(error_msg) => {
                return Err(format!("Error getting ASG: {}", error_msg).into());
            }
        }

        // Show what we're about to change
        output.stderr("Scaling to:");
        if let Some(min) = self.min_size {
            output.stderr(&format!("min size: {}", min));
        }
        if let Some(max) = self.max_size {
            output.stderr(&format!("max size: {}", max));
        }
        if let Some(desired) = self.desired_capacity {
            output.stderr(&format!("desired capacity: {}", desired));
        }

        // Confirmation prompt (unless --yes flag is used)
        if !self.yes {
            let prompt = format!("WARNING: You are about to scale ASG {}. Continue?", name);
            if !Confirm::new()
                .with_prompt(prompt)
                .interact()
                .unwrap_or(false)
            {
                output.stderr("Scaling cancelled.");
                return Ok(output);
            }
        }

        // Perform the scaling operation
        match client
            .update_auto_scaling_group()
            .set_auto_scaling_group_name(Some(name.to_string()))
            .set_min_size(self.min_size)
            .set_max_size(self.max_size)
            .set_desired_capacity(self.desired_capacity)
            .send()
            .await
        {
            Ok(_) => {
                output.stderr("Scaling operation completed successfully");

                // Show ASG state after scaling
                output.stderr("ASG State AFTER Scaling:");
                match get_asg_details(client, &name, opts.verbose).await {
                    Ok(asg) => {
                        output.stdout(&serde_json::to_string_pretty(&asg).unwrap());
                    }
                    Err(error_msg) => {
                        output.stderr(&format!(
                            "Warning: Error getting ASG after scaling: {}",
                            error_msg
                        ));
                    }
                }
                Ok(output)
            }
            Err(err) => {
                use aws_sdk_autoscaling::error::SdkError;
                let error_msg = match err {
                    SdkError::ServiceError(service_err) => {
                        handle_service_error(service_err.err(), "scale ASG")
                    }
                    SdkError::DispatchFailure(dispatch_err) => {
                        handle_dispatch_failure(&dispatch_err, "scale ASG")
                    }
                    SdkError::ConstructionFailure(err) => {
                        handle_construction_failure(&err, "scale ASG", opts.verbose)
                    }
                    SdkError::TimeoutError(err) => {
                        handle_timeout_error(&err, "scale ASG", opts.verbose)
                    }
                    SdkError::ResponseError(err) => {
                        handle_response_error(&err, "scale ASG", opts.verbose)
                    }
                    _ => handle_unknown_error(&err, "scale ASG", opts.verbose),
                };
                Err(error_msg.into())
            }
        }
    }
}

impl DetachInstancesArg {
    async fn execute(
        self,
        name: String,
        client: &Client,
        opts: &GlobalOptions,
    ) -> Result<Output, Box<dyn std::error::Error>> {
        match client
            .detach_instances()
            .set_instance_ids(Some(self.ids))
            .set_auto_scaling_group_name(Some(name))
            .should_decrement_desired_capacity(!self.replace)
            .send()
            .await
        {
            Ok(response) => {
                let result = json!(
                    response
                        .activities()
                        .iter()
                        .map(|activity| {
                            return json!({
                                "description": activity.description(),
                                "id": activity.activity_id(),
                                "cause": activity.cause(),
                                "time": format!("{:?} - {:?}",
                                    activity.start_time().map(|dt|{dt.to_string()}).unwrap_or("N/A".to_string()),
                                    activity.end_time().map(|dt|{dt.to_string()}).unwrap_or("N/A".to_string()),
                                ),
                                "status": activity.status_message()
                            });
                        })
                        .collect::<Vec<Value>>()
                );
                let output = Output::new(opts.verbose);
                output.stdout(&serde_json::to_string_pretty(&result).unwrap());
                Ok(output)
            }
            Err(err) => {
                use aws_sdk_autoscaling::error::SdkError;
                let error_msg = match err {
                    SdkError::ServiceError(service_err) => {
                        handle_service_error(service_err.err(), "detach instances")
                    }
                    SdkError::DispatchFailure(dispatch_err) => {
                        handle_dispatch_failure(&dispatch_err, "detach instances")
                    }
                    SdkError::ConstructionFailure(err) => {
                        handle_construction_failure(&err, "detach instances", opts.verbose)
                    }
                    SdkError::TimeoutError(err) => {
                        handle_timeout_error(&err, "detach instances", opts.verbose)
                    }
                    SdkError::ResponseError(err) => {
                        handle_response_error(&err, "detach instances", opts.verbose)
                    }
                    _ => handle_unknown_error(&err, "detach instances", opts.verbose),
                };
                Err(error_msg.into())
            }
        }
    }
}

impl AttachInstancesArg {
    async fn execute(
        self,
        name: String,
        client: &Client,
        opts: &GlobalOptions,
    ) -> Result<Output, Box<dyn std::error::Error>> {
        match client
            .attach_instances()
            .set_instance_ids(Some(self.ids))
            .set_auto_scaling_group_name(Some(name))
            .send()
            .await
        {
            Ok(_) => {
                let output = Output::new(opts.verbose);
                output.stderr("done");
                Ok(output)
            }
            Err(err) => {
                use aws_sdk_autoscaling::error::SdkError;
                let error_msg = match err {
                    SdkError::ServiceError(service_err) => {
                        handle_service_error(service_err.err(), "attach instances")
                    }
                    SdkError::DispatchFailure(dispatch_err) => {
                        handle_dispatch_failure(&dispatch_err, "attach instances")
                    }
                    SdkError::ConstructionFailure(err) => {
                        handle_construction_failure(&err, "attach instances", opts.verbose)
                    }
                    SdkError::TimeoutError(err) => {
                        handle_timeout_error(&err, "attach instances", opts.verbose)
                    }
                    SdkError::ResponseError(err) => {
                        handle_response_error(&err, "attach instances", opts.verbose)
                    }
                    _ => handle_unknown_error(&err, "attach instances", opts.verbose),
                };
                Err(error_msg.into())
            }
        }
    }
}
