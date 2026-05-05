use crate::commands::Output;
use crate::commands::aws::{
    arg::GlobalOptions,
    elb::arg::{AWSElbCommand, ElbSubCommand, GetArg, GetListenersArg, GetRulesArg},
    error::{
        handle_construction_failure, handle_dispatch_failure, handle_response_error,
        handle_service_error, handle_timeout_error, handle_unknown_error,
    },
};
use aws_sdk_elasticloadbalancingv2::{
    Client,
    types::{
        Action, FixedResponseActionConfig, Listener, LoadBalancer, RedirectActionConfig, Rule,
    },
};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::process;

impl AWSElbCommand {
    pub async fn execute(self, opts: GlobalOptions) -> Result<Output, Box<dyn std::error::Error>> {
        let client = Client::new(&opts.sdk_config);
        match self.command {
            ElbSubCommand::Get(args) => args.execute(&client, opts).await,
            ElbSubCommand::GetRules(args) => args.execute(&client, opts).await,
            ElbSubCommand::GetListeners(args) => args.execute(&client, opts).await,
        }
    }
}

impl GetArg {
    async fn execute(
        self,
        client: &Client,
        opts: GlobalOptions,
    ) -> Result<Output, Box<dyn std::error::Error>> {
        let output = Output::new(opts.verbose);
        if opts.verbose {
            output.stderr("Searching for ELB...");
        }
        let matched_lbs = if !self.fuzzy {
            // For exact search, use the original name (case-sensitive)
            exact_search(client, self.name).await?
        } else {
            // For fuzzy search, use lowercase for case-insensitive matching
            fuzzy_search(client, self.name.to_lowercase(), self.num).await?
        };

        if opts.verbose {
            output.stderr(&format!(
                "Found {} matching load balancer(s)",
                matched_lbs.len()
            ));
        }

        let lb_data: Vec<Value> = matched_lbs
            .iter()
            .map(|lb| if opts.verbose { lb.long() } else { lb.short() })
            .collect();

        output.stdout(&serde_json::to_string_pretty(&lb_data).unwrap());
        Ok(output)
    }
}

// exact_search performs search with exact name
// Note: AWS doesn't support pagination when specifying load balancer names
async fn exact_search(
    client: &Client,
    name: String,
) -> Result<Vec<LoadBalancer>, Box<dyn std::error::Error>> {
    let output = Output::new(false);
    let request = client
        .describe_load_balancers()
        .set_names(Some(vec![name.clone()]))
        .send()
        .await;
    match request {
        Ok(response) => Ok(response.load_balancers().to_vec()),
        Err(err) => {
            use aws_sdk_elasticloadbalancingv2::error::SdkError;
            // Check if it's a LoadBalancerNotFound error before consuming err
            let is_not_found = matches!(
                &err,
                SdkError::ServiceError(se)
                    if se.err().is_load_balancer_not_found_exception()
            );

            if is_not_found {
                output.stderr(&format!("ALB: {name} not found"));
                output.stderr("Tip: Try without --exact flag for partial/fuzzy matching.");
                // if the load balancers not found, just return empty vec
                return Ok(vec![]);
            }

            match err {
                SdkError::ServiceError(service_err) => {
                    handle_service_error(service_err.err(), "describe load balancers");
                }
                SdkError::DispatchFailure(dispatch_err) => {
                    handle_dispatch_failure(&dispatch_err, "describe load balancers");
                }
                SdkError::ConstructionFailure(err) => {
                    handle_construction_failure(&err, "describe load balancers", false);
                }
                SdkError::TimeoutError(err) => {
                    handle_timeout_error(&err, "describe load balancers", false);
                }
                SdkError::ResponseError(err) => {
                    handle_response_error(&err, "describe load balancers", false);
                }
                _ => {
                    handle_unknown_error(&err, "describe load balancers", false);
                }
            }

            process::exit(1);
        }
    }
}

async fn fuzzy_search(
    client: &Client,
    name: String,
    limit: Option<i8>,
) -> Result<Vec<LoadBalancer>, Box<dyn std::error::Error>> {
    let mut matched_lbs = Vec::new();
    let mut next_marker: Option<String> = None;
    let page_size = 400; // AWS max is 400

    // Fetch load balancers with pagination
    // When limit is None, fetch all pages; when limit is Some, stop early if we have enough
    'read_loop: loop {
        let mut request = client
            .describe_load_balancers()
            .set_page_size(Some(page_size));

        if let Some(marker) = next_marker {
            request = request.set_marker(Some(marker));
        }

        match request.send().await {
            Ok(response) => {
                // Filter matching load balancers from this page
                let page_matches = response
                    .load_balancers()
                    .to_vec()
                    .iter()
                    .filter(|lb| {
                        lb.load_balancer_name()
                            .unwrap_or("")
                            .to_lowercase()
                            .contains(&name)
                    })
                    .cloned()
                    .collect::<Vec<_>>();

                matched_lbs.extend(page_matches);

                // If limit is specified, check if we have enough results and stop early
                if let Some(limit) = limit {
                    let limit = limit as usize;
                    if matched_lbs.len() >= limit {
                        matched_lbs.truncate(limit);
                        break 'read_loop;
                    }
                }

                // Check if there are more pages
                if let Some(marker) = response.next_marker() {
                    next_marker = Some(marker.to_string());
                } else {
                    // No more pages, we've fetched everything
                    break;
                }
            }
            Err(err) => {
                use aws_sdk_elasticloadbalancingv2::error::SdkError;

                match err {
                    SdkError::ServiceError(service_err) => {
                        handle_service_error(service_err.err(), "get ELB");
                    }
                    SdkError::DispatchFailure(dispatch_err) => {
                        handle_dispatch_failure(&dispatch_err, "get ELB");
                    }
                    SdkError::ConstructionFailure(err) => {
                        handle_construction_failure(&err, "get ELB", false);
                    }
                    SdkError::TimeoutError(err) => {
                        handle_timeout_error(&err, "get ELB", false);
                    }
                    SdkError::ResponseError(err) => {
                        handle_response_error(&err, "get ELB", false);
                    }
                    _ => {
                        handle_unknown_error(&err, "get ELB", false);
                    }
                }

                process::exit(1);
            }
        }
    }
    Ok(matched_lbs)
}

// Extension trait to add formatting methods to LoadBalancer
trait LoadBalancerExt {
    fn short(&self) -> Value;
    fn long(&self) -> Value;
    fn console_url(&self) -> String;
}

impl LoadBalancerExt for LoadBalancer {
    fn console_url(&self) -> String {
        // Extract region and ARN to build AWS console URL
        if let Some(arn) = self.load_balancer_arn() {
            // ARN format: arn:aws:elasticloadbalancing:region:account:loadbalancer/type/name/id
            let parts: Vec<&str> = arn.split(':').collect();
            if parts.len() >= 4 {
                let region = parts[3];
                // URL encode the ARN for use in the console URL
                let encoded_arn = urlencoding::encode(arn);
                return format!(
                    "https://{}.console.aws.amazon.com/ec2/home?region={}#LoadBalancer:loadBalancerArn={}",
                    region, region, encoded_arn
                );
            }
        }
        "N/A".to_string()
    }

    fn short(&self) -> Value {
        json!({
            "Name": self.load_balancer_name().unwrap_or("N/A"),
            "Arn": self.load_balancer_arn().unwrap_or("N/A"),
            "_url": self.console_url()
        })
    }

    fn long(&self) -> Value {
        json!({
            "Arn": self.load_balancer_arn().unwrap_or("N/A"),
            "Name": self.load_balancer_name().unwrap_or("N/A"),
            "DNSName": self.dns_name().unwrap_or("N/A"),
            "Scheme": self.scheme().map(|s| s.as_str()).unwrap_or("N/A"),
            "VpcId": self.vpc_id().unwrap_or("N/A"),
            "State": self.state()
                .and_then(|s| s.code())
                .map(|c| c.as_str())
                .unwrap_or("N/A"),
            "Type": self.r#type().map(|t| t.as_str()).unwrap_or("N/A"),
            "AvailabilityZones": self.availability_zones()
                .iter()
                .map(|az| az.zone_name().unwrap_or("N/A"))
                .collect::<Vec<_>>(),
            "SecurityGroups": self.security_groups(),
            "IpAddressType": self.ip_address_type().map(|t| t.as_str()).unwrap_or("N/A"),
            "_url": self.console_url()
        })
    }
}

impl GetRulesArg {
    pub async fn execute(
        self,
        client: &Client,
        opts: GlobalOptions,
    ) -> Result<Output, Box<dyn std::error::Error>> {
        let output = Output::new(opts.verbose);

        // Fetch all rules from the listener
        let rules = client
            .describe_rules()
            .set_listener_arn(Some(self.listener_arn.clone()))
            .send()
            .await
            .map_err(|err| -> Box<dyn std::error::Error> {
                use aws_sdk_elasticloadbalancingv2::error::SdkError;
                let error_msg = match err {
                    SdkError::ServiceError(service_err) => {
                        handle_service_error(service_err.err(), "describe rules for listener")
                    }
                    SdkError::DispatchFailure(dispatch_err) => {
                        handle_dispatch_failure(&dispatch_err, "describe rules for listener")
                    }
                    SdkError::ConstructionFailure(err) => handle_construction_failure(
                        &err,
                        "describe rules for listener",
                        opts.verbose,
                    ),
                    SdkError::TimeoutError(err) => {
                        handle_timeout_error(&err, "describe rules for listener", opts.verbose)
                    }
                    SdkError::ResponseError(err) => {
                        handle_response_error(&err, "describe rules for listener", opts.verbose)
                    }
                    _ => handle_unknown_error(&err, "describe rules for listener", opts.verbose),
                };
                format!(
                    "Error: Failed to describe rules for listener: {}",
                    error_msg
                )
                .into()
            })?
            .rules()
            .to_vec();

        if rules.is_empty() {
            if opts.verbose {
                output.stderr(&format!(
                    "No rules found for listener: {}",
                    self.listener_arn
                ));
            }
            output.stdout("[]");
            return Ok(output);
        }

        // Get rule ARNs for tag lookup
        let rule_arns: Vec<String> = rules
            .iter()
            .filter_map(|rule| rule.rule_arn().map(|s| s.to_string()))
            .collect();

        // Fetch tags for all rules only if we are running with verbose or we have the tag_filters
        let rule_tags = if rule_arns.is_empty() {
            if opts.verbose {
                output.stderr("Warning: No valid rule ARNs found");
            }
            Some(HashMap::new())
        } else if opts.verbose || self.tag.is_some() {
            match fetch_rule_tags(client, &rule_arns).await {
                Ok(tags) => Some(tags),
                Err(err) => {
                    return Err(format!("Error: Failed to fetch tags for rules: {}", err).into());
                }
            }
        } else {
            None
        };

        // Filter rules by tags if specified
        let filtered_rules = if let Some(tag_filters) = &self.tag {
            // Parse tag filters into key-value pairs
            let required_tags = parse_tag_filters(tag_filters);

            if opts.verbose {
                output.stderr(&format!("Filtering rules with tags: {:?}", required_tags));
            }

            // Filter rules that have ALL required tags
            // rule_tags must be Some here because we fetch it when tag filter is specified
            if let Some(ref tags_map) = rule_tags {
                rules
                    .into_iter()
                    .filter(|rule| {
                        if let Some(arn) = rule.rule_arn()
                            && let Some(tags) = tags_map.get::<str>(arn)
                        {
                            return has_all_required_tags(tags, &required_tags);
                        }
                        false
                    })
                    .collect()
            } else {
                // This shouldn't happen, but handle it gracefully
                output.stderr("Warning: Tag filtering requested but tags were not fetched");
                rules
            }
        } else {
            rules
        };

        // Apply limit if specified
        let final_rules: Vec<Rule> = match self.num {
            Some(limit) => filtered_rules.into_iter().take(limit as usize).collect(),
            None => filtered_rules,
        };

        if opts.verbose {
            output.stderr(&format!("Found {} matching rule(s)", final_rules.len()));
        }

        // Format output with tags
        let rule_data: Vec<Value> = if opts.verbose {
            final_rules
                .iter()
                .map(|rule| {
                    let tags = if let Some(ref tags_map) = rule_tags {
                        rule.rule_arn()
                            .and_then(|arn| tags_map.get::<str>(arn))
                            .cloned()
                            .unwrap_or_default()
                    } else {
                        HashMap::new()
                    };
                    rule.long(&tags)
                })
                .collect()
        } else {
            final_rules.iter().map(|rule| rule.short()).collect()
        };

        output.stdout(&serde_json::to_string_pretty(&rule_data).unwrap());
        Ok(output)
    }
}

// Parse tag filters from command line arguments
// Input: ["key1", "value1", "key2", "value2"]
// Output: HashMap { "key1" => "value1", "key2" => "value2" }
fn parse_tag_filters(tag_filters: &[String]) -> HashMap<String, String> {
    let mut tags = HashMap::new();
    for chunk in tag_filters.chunks(2) {
        if chunk.len() == 2 {
            tags.insert(chunk[0].clone(), chunk[1].clone());
        }
    }
    tags
}

// Fetch tags for multiple rule ARNs and return a map of ARN -> tags
async fn fetch_rule_tags(
    client: &Client,
    rule_arns: &[String],
) -> Result<HashMap<String, HashMap<String, String>>, String> {
    let mut result = HashMap::new();

    // 20 is fiexed hard limit of number of resources that we could get tags
    // at a time.
    for rule_arns in rule_arns.chunks(20) {
        let response = client
            .describe_tags()
            .set_resource_arns(Some(rule_arns.to_vec()))
            .send()
            .await
            .map_err(|e| format!("Failed to describe tags: {e:?}"))?;

        for tag_desc in response.tag_descriptions() {
            if let Some(arn) = tag_desc.resource_arn() {
                let mut tags = HashMap::new();
                for tag in tag_desc.tags() {
                    if let (Some(key), Some(value)) = (tag.key(), tag.value()) {
                        tags.insert(key.to_string(), value.to_string());
                    }
                }
                result.insert(arn.to_string(), tags);
            }
        }
    }

    Ok(result)
}

// Check if a rule has all required tags with matching values
fn has_all_required_tags(
    rule_tags: &HashMap<String, String>,
    required_tags: &HashMap<String, String>,
) -> bool {
    required_tags
        .iter()
        .all(|(key, value)| rule_tags.get(key) == Some(value))
}

// Extension trait to add formatting methods to Rule
trait RuleExt {
    fn short(&self) -> Value;
    fn long(&self, tags: &HashMap<String, String>) -> Value;
    fn console_url(&self) -> String;
    fn format_conditions(&self) -> HashMap<String, String>;
    fn format_actions(&self) -> Value;
}

impl RuleExt for Rule {
    fn console_url(&self) -> String {
        // Extract region from ARN to build AWS console URL
        if let Some(arn) = self.rule_arn() {
            // ARN format: arn:aws:elasticloadbalancing:region:account:listener-rule/app/name/id/id
            let parts: Vec<&str> = arn.split(':').collect();
            if parts.len() >= 4 {
                let region = parts[3];
                // URL encode the ARN for use in the console URL
                let encoded_arn = urlencoding::encode(arn);
                return format!(
                    "https://{}.console.aws.amazon.com/ec2/home?region={}#ListenerRuleDetails:ruleArn={}",
                    region, region, encoded_arn
                );
            }
        }
        "N/A".to_string()
    }

    fn short(&self) -> Value {
        json!({
            "Arn": self.rule_arn().unwrap_or("N/A"),
            "Priority": self.priority().unwrap_or("N/A"),
            "_url": self.console_url()
        })
    }

    fn long(&self, tags: &HashMap<String, String>) -> Value {
        let conditions = self.format_conditions();
        let actions = self.format_actions();

        json!({
            "Arn": self.rule_arn().unwrap_or("N/A"),
            "Priority": self.priority().unwrap_or("N/A"),
            "IsDefault": self.is_default().unwrap_or(false),
            "Conditions": conditions,
            "Actions": actions,
            "Tags": tags,
            "_url": self.console_url()
        })
    }

    fn format_conditions(&self) -> HashMap<String, String> {
        self.conditions()
            .iter()
            .map(|c| (c.field().unwrap_or("N/A").to_string(), c.values().join(",")))
            .collect()
    }

    fn format_actions(&self) -> Value {
        let actions: Vec<Value> = self.actions().iter().map(|a| a.to_value()).collect();

        // If there's only one action, return it directly, otherwise return array
        if actions.len() == 1 {
            actions.into_iter().next().unwrap()
        } else {
            json!(actions)
        }
    }
}

// Extension trait for RedirectActionConfig
trait RedirectActionConfigExt {
    fn to_string(&self) -> String;
}

impl RedirectActionConfigExt for RedirectActionConfig {
    fn to_string(&self) -> String {
        let protocol = self.protocol().unwrap_or("#{protocol}");
        let host = self.host().unwrap_or("#{host}");
        let port = self.port().unwrap_or("#{port}");
        let path = self.path().unwrap_or("#{path}");
        let query = self.query().unwrap_or("#{query}");
        let status_code = self.status_code().map(|s| s.as_str()).unwrap_or("N/A");

        format!(
            "{}://{}:{}{}{} ({})",
            protocol,
            host,
            port,
            path,
            if query.is_empty() {
                String::new()
            } else {
                format!("?{}", query)
            },
            status_code
        )
    }
}
trait FixedResponseActionConfigExt {
    fn to_string(&self) -> String;
}
impl FixedResponseActionConfigExt for FixedResponseActionConfig {
    fn to_string(&self) -> String {
        format!(
            "Status: {}, ContentType: {}, Body: {}",
            self.status_code().unwrap_or("N/A"),
            self.content_type().unwrap_or("N/A"),
            self.message_body().unwrap_or("N/A"),
        )
    }
}

trait ActionExt {
    fn to_value(&self) -> Value;
}

impl ActionExt for Action {
    fn to_value(&self) -> Value {
        // Transform actions into structured format with type and config
        if let Some(forward_config) = self.forward_config() {
            let config: HashMap<String, String> = forward_config
                .target_groups()
                .iter()
                .filter_map(|tg| {
                    let arn = tg.target_group_arn()?;
                    let weight = tg.weight().unwrap_or(0);
                    Some((arn.to_string(), weight.to_string()))
                })
                .collect();

            json!({
                "type": "forward",
                "config": config
            })
        } else if let Some(redirect_config) = self.redirect_config() {
            json!({
                "type": "redirect",
                "config": redirect_config.to_string()
            })
        } else if let Some(fixed_response_config) = self.fixed_response_config() {
            json!({
                "type": "fixed_response",
                "config": fixed_response_config.to_string()
            })
        } else {
            json!({
                "type": "unknown",
                "config": {}
            })
        }
    }
}
impl GetListenersArg {
    async fn execute(
        self,
        client: &Client,
        opts: GlobalOptions,
    ) -> Result<Output, Box<dyn std::error::Error>> {
        let output = Output::new(opts.verbose);

        match client
            .describe_listeners()
            .set_load_balancer_arn(Some(self.loadbalancer_arn))
            .send()
            .await
        {
            Ok(response) => match response.listeners {
                Some(listeners) => {
                    let listener_data: Vec<Value> = listeners
                        .iter()
                        .map(|l| if opts.verbose { l.long() } else { l.short() })
                        .collect();

                    output.stdout(&serde_json::to_string_pretty(&listener_data).unwrap());
                    return Ok(output);
                }
                None => {
                    output.stderr("Empty listener");
                    return Ok(output);
                }
            },
            Err(err) => {
                use aws_sdk_elasticloadbalancingv2::error::SdkError;

                match err {
                    SdkError::ServiceError(ref service_err) => {
                        handle_service_error(service_err.err(), "get listener");
                    }
                    SdkError::DispatchFailure(ref dispatch_err) => {
                        handle_dispatch_failure(dispatch_err, "get listener");
                    }
                    SdkError::ConstructionFailure(ref err) => {
                        handle_construction_failure(err, "get listener", opts.verbose);
                    }
                    SdkError::TimeoutError(ref err) => {
                        handle_timeout_error(err, "get listener", opts.verbose);
                    }
                    SdkError::ResponseError(ref err) => {
                        handle_response_error(err, "get listener", opts.verbose);
                    }
                    _ => {
                        handle_unknown_error(&err, "get listener", opts.verbose);
                    }
                }

                Err(format!("Error: Failed to get listener: {}", err).into())
            }
        }
    }
}

trait ListenerExt {
    fn long(&self) -> Value;
    fn short(&self) -> Value;
}
impl ListenerExt for Listener {
    fn long(&self) -> Value {
        json!({
            "Arn": self.listener_arn().unwrap_or("N/A"),
            "LoadBalancerArn": self.load_balancer_arn().unwrap_or("N/A"),
            "Port": self.port(),
            "Protocol": self.protocol().map(|p| p.as_str()),
            "Certificates": self.certificates()
                .iter()
                .map(|cert| json!({
                    "CertificateArn": cert.certificate_arn(),
                    "IsDefault": cert.is_default()
                }))
                .collect::<Vec<_>>(),
            "SslPolicy": self.ssl_policy(),
            "DefaultActions": self.default_actions()
                .iter()
                .map(|a| json!({
                    "Type": a.r#type().map(|t| t.as_str()),
                    "TargetGroupArn": a.target_group_arn(),
                    "Order": a.order()
                }))
                .collect::<Vec<_>>(),
            "AlpnPolicy": self.alpn_policy()
        })
    }
    fn short(&self) -> Value {
        json!({
            "Arn": self.listener_arn().unwrap_or("N/A"),
            "Port": self.port().map(|p| p.to_string()).unwrap_or("N/A".to_string()),
            "Protocol": self.protocol().map(|p| p.as_str()).unwrap_or("N/A")
        })
    }
}
