use aws_sdk_route53::{
    Client,
    types::{HostedZone, ResourceRecordSet},
};
use serde_json::{Value, json};

use crate::commands::Output;
use crate::commands::aws::{
    arg::GlobalOptions,
    error::{
        handle_construction_failure, handle_dispatch_failure, handle_response_error,
        handle_service_error, handle_timeout_error, handle_unknown_error,
    },
    route53::arg::{AWSRoute53Command, GetArg, Route53SubCommand},
};

impl AWSRoute53Command {
    pub async fn execute(self, opts: GlobalOptions) -> Result<Output, Box<dyn std::error::Error>> {
        let client = Client::new(&opts.sdk_config);
        match self.command {
            Route53SubCommand::Get(args) => args.execute(&client, opts).await,
        }
    }
}

impl GetArg {
    async fn execute(
        self,
        client: &Client,
        opts: GlobalOptions,
    ) -> Result<Output, Box<dyn std::error::Error>> {
        // Basic domain validation
        if self.dns_name.is_empty() || !self.dns_name.contains('.') {
            return Err(format!(
                "Error: Invalid domain format. Please provide a valid domain name."
            )
            .into());
        }

        // Find the appropriate hosted zone by trying different domain levels
        let mut output = Output::new(opts.verbose);
        let hosted_zone =
            Self::find_hosted_zone_for_domain(client, &self.dns_name, &opts, &mut output).await?;

        // Get resource records for the domain
        match client
            .list_resource_record_sets()
            .set_hosted_zone_id(Some(hosted_zone.id().to_string()))
            .set_start_record_name(Some(self.dns_name.clone()))
            .send()
            .await
        {
            Ok(response) => {
                let records: Vec<&ResourceRecordSet> = response
                    .resource_record_sets()
                    .iter()
                    .filter(|record| {
                        let name = record.name();
                        name == &self.dns_name || name == &format!("{}.", self.dns_name)
                    })
                    .collect();

                let record_values: Vec<Value> = records
                    .iter()
                    .map(|r| if opts.verbose { r.long() } else { r.short() })
                    .collect();

                let result = json!({
                    "hosted_zone": if opts.verbose {hosted_zone.long()}else{hosted_zone.short()},
                    "records": record_values,
                });

                output.stdout(&serde_json::to_string_pretty(&result).unwrap());
                Ok(output)
            }
            Err(err) => {
                use aws_sdk_route53::error::SdkError;
                let is_not_found =
                    matches!(&err, SdkError::ServiceError(se) if se.err().is_no_such_hosted_zone());
                if is_not_found {
                    output.stderr(&format!("Route53: {} not found", self.dns_name));
                }
                match err {
                    SdkError::ServiceError(ref service_err) => {
                        handle_service_error(service_err.err(), "get route53 record");
                    }
                    SdkError::DispatchFailure(ref dispatch_err) => {
                        handle_dispatch_failure(dispatch_err, "get route53 record");
                    }
                    SdkError::ConstructionFailure(ref err) => {
                        handle_construction_failure(err, "get route53 record", false);
                    }
                    SdkError::TimeoutError(ref err) => {
                        handle_timeout_error(err, "get route53 record", false);
                    }
                    SdkError::ResponseError(ref err) => {
                        handle_response_error(err, "get route53 record", false);
                    }
                    _ => {
                        handle_unknown_error(&err, "get route53 record", false);
                    }
                }
                Err(format!("Error: Failed to get route53 record: {}", err).into())
            }
        }
    }

    async fn find_hosted_zone_for_domain(
        client: &Client,
        domain: &str,
        opts: &GlobalOptions,
        output: &mut Output,
    ) -> Result<HostedZone, Box<dyn std::error::Error>> {
        // Try to find hosted zone by checking domain and its parent domains
        let domain_parts: Vec<&str> = domain.split('.').collect();

        for i in (0..domain_parts.len() - 1).rev() {
            let candidate_domain = domain_parts[i..].join(".");
            let candidate_domain_with_dot = format!("{}.", candidate_domain);

            if opts.verbose {
                output.stderr(&format!(
                    "Trying to find hosted zone for: {}",
                    candidate_domain
                ));
            }

            match client
                .list_hosted_zones_by_name()
                .set_dns_name(Some(candidate_domain_with_dot.clone()))
                .set_max_items(Some(1))
                .send()
                .await
            {
                Ok(response) => {
                    let zones = response.hosted_zones();
                    for zone in zones {
                        if zone.name() == &candidate_domain_with_dot {
                            if opts.verbose {
                                output.stderr(&format!(
                                    "Found hosted zone: {} ({})",
                                    zone.name(),
                                    zone.id()
                                ));
                            }
                            return Ok(zone.clone());
                        }
                    }
                }
                Err(err) => {
                    if opts.verbose {
                        output.stderr(&format!(
                            "Failed to check domain '{}': {}",
                            candidate_domain, err
                        ));
                    }
                }
            }
        }

        output.stderr(&format!(
            "Error: No hosted zone found for domain '{}' or any of its parent domains",
            domain
        ));
        output.stderr("Make sure the domain is managed by Route53 in your AWS account.");
        Err(format!(
            "Error: No hosted zone found for domain '{}' or any of its parent domains",
            domain
        )
        .into())
    }
}

trait HostedZoneExt {
    fn short(&self) -> Value;
    fn long(&self) -> Value;
}
impl HostedZoneExt for HostedZone {
    fn short(&self) -> Value {
        json!({
            "ID": self.id(),
            "Name": self.name(),
            "RecordCount": self.resource_record_set_count().map(|c| c.to_string()).unwrap_or("N/A".to_string()),
            "_url": format!("https://us-east-1.console.aws.amazon.com/route53/v2/hostedzones#ListRecordSets/{}", self.id().trim_start_matches("/hostedzone/"))
        })
    }
    fn long(&self) -> Value {
        let mut record_jsons = self.short();
        if let Some(config) = self.config() {
            record_jsons["config"] = json!({
                "Description": config.comment(),
                "PrivateZone": config.private_zone()
            });
        };
        record_jsons
    }
}

trait ResourceRecordSetExt {
    fn short(&self) -> Value;
    fn long(&self) -> Value;
}

impl ResourceRecordSetExt for ResourceRecordSet {
    fn short(&self) -> Value {
        let name = self.name();
        let record_type = self.r#type();
        let ttl = self.ttl().unwrap_or(0);

        let mut record_json = json!({
            "Name": name,
            "Type": record_type.as_str(),
            "TTL": ttl,
        });

        let records = self.resource_records();
        if !records.is_empty() {
            let values: Vec<&str> = records.iter().map(|rr| rr.value()).collect();
            record_json["Values"] = json!(values);
        }

        if let Some(weight) = self.weight() {
            record_json["Weight"] = json!(weight);
        }
        if let Some(alias_target) = self.alias_target() {
            record_json["Target"] = json!(alias_target.dns_name());
        }
        record_json
    }

    fn long(&self) -> Value {
        let mut record_json = self.short();
        if let Some(alias_target) = self.alias_target() {
            // override the target with details
            record_json["Target"] = json!({
                "DNSName": alias_target.dns_name(),
                "HostedZoneId": alias_target.hosted_zone_id(),
                "EvaluateTargetHealth": alias_target.evaluate_target_health()
            });
        }
        if let Some(health_check_id) = self.health_check_id() {
            record_json["HealthCheckId"] = json!(health_check_id);
        }
        if let Some(region) = self.region() {
            record_json["Region"] = json!(region.to_string());
        }
        record_json
    }
}
