use crate::commands::Output;
use crate::commands::aws::{arg::GlobalOptions, whoami::arg::AWSWhoAmICommand};
use aws_sdk_sts::{Client, meta::PKG_VERSION};
use serde_json::json;

impl AWSWhoAmICommand {
    pub async fn execute(self, opts: GlobalOptions) -> Result<Output, Box<dyn std::error::Error>> {
        let client = Client::new(&opts.sdk_config);

        let response = client
            .get_caller_identity()
            .send()
            .await
            .map_err(|err| format!("Error: whoami failed, {}", err))?;

        // Create simple output
        let output = Output::new(opts.verbose);

        // Add debug information
        output.stderr(&format!("STS client version: {}", PKG_VERSION));
        output.stderr(&format!(
            "Region: {}",
            opts.sdk_config.region().unwrap().as_ref()
        ));

        // Set JSON data
        let data = json!({
            "UserId": response.user_id().unwrap_or("N/A"),
            "Account": response.account().unwrap_or("N/A"),
            "Arn": response.arn().unwrap_or("N/A"),
        });

        output.stdout(&serde_json::to_string_pretty(&data).unwrap());

        Ok(output)
    }
}
