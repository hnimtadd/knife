use aws_sdk_ssm::Client;
use serde_json::json;
use std::process::{self, Command, Stdio};
use tokio::signal;

use crate::commands::Output;
use crate::commands::aws::{
    arg::GlobalOptions,
    error::{
        handle_construction_failure, handle_dispatch_failure, handle_response_error,
        handle_service_error, handle_timeout_error, handle_unknown_error,
    },
    ssm::arg::{AWSSSMCommand, SSMSubCommand, StartArg},
};

impl AWSSSMCommand {
    pub async fn execute(self, opts: GlobalOptions) -> Result<Output, Box<dyn std::error::Error>> {
        let client = Client::new(&opts.sdk_config);
        match self.command {
            SSMSubCommand::Start(args) => args.execute(&client, opts).await,
        }
    }
}

impl StartArg {
    async fn execute(
        self,
        client: &Client,
        opts: GlobalOptions,
    ) -> Result<Output, Box<dyn std::error::Error>> {
        // Support reading from stdin if arn is "-" or empty
        let arn = if self.id == "-" {
            let mut input = String::new();
            if let Err(e) = std::io::stdin().read_line(&mut input) {
                return Err(format!("Error reading from stdin: {}", e).into());
            }
            input.trim().to_string()
        } else {
            self.id.clone()
        };

        if arn.is_empty() {
            return Err(format!("Error: Invalid instance ARN.").into());
        }

        // Check if session-manager-plugin is installed
        if !is_plugin_installed() {
            return Err(format!("Error: session-manager-plugin is not installed.").into());
        }

        // Start the session
        match client
            .start_session()
            .set_target(Some(self.id.clone()))
            .send()
            .await
        {
            Ok(response) => {
                let session_id = response.session_id().unwrap_or_default();
                let token_value = response.token_value().unwrap_or_default();
                let stream_url = response.stream_url().unwrap_or_default();

                // Prepare parameters for session-manager-plugin
                let session_params = json!({
                    "SessionId": session_id,
                    "TokenValue": token_value,
                    "StreamUrl": stream_url,
                });

                let start_session_params = json!({
                    "Target": self.id,
                });

                // Get region from SDK config
                let region = opts
                    .sdk_config
                    .region()
                    .map(|r| r.as_ref())
                    .unwrap_or("us-east-1");

                // Invoke session-manager-plugin
                let mut command = Command::new("session-manager-plugin");
                command
                    .arg(session_params.to_string())
                    .arg(region)
                    .arg("StartSession")
                    .arg("knife") // profile name (can be any identifier)
                    .arg(start_session_params.to_string())
                    .stdin(Stdio::inherit())
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit());

                // Start the session-manager-plugin process
                let mut child = match command.spawn() {
                    Ok(child) => child,
                    Err(e) => {
                        return Err(format!("Failed to start interactive session: {}", e).into());
                    }
                };

                // Set up signal handling to forward signals to the child process
                // This mimics the behavior of the native AWS CLI
                let child_id = child.id();

                // Create a signal handler that forwards common termination signals
                tokio::spawn(async move {
                    // Handle SIGINT (Ctrl+C)
                    let ctrl_c = signal::ctrl_c();
                    let mut sigterm = signal::unix::signal(libc::SIGTERM.into()).unwrap();
                    let mut sighup = signal::unix::signal(libc::SIGHUP.into()).unwrap();

                    tokio::select! {
                        _ = ctrl_c => {
                            // Forward SIGINT to child process
                            unsafe { libc::kill(child_id as i32, libc::SIGINT); }
                        }
                        _ = sigterm.recv() => {
                            // Forward SIGTERM to child process
                            unsafe { libc::kill(child_id as i32, libc::SIGTERM); }
                        }
                        _ = sighup.recv() => {
                            // Forward SIGHUP to child process
                            unsafe { libc::kill(child_id as i32, libc::SIGHUP); }
                        }
                    }
                });

                // Wait for the child process to complete
                match child.wait() {
                    Ok(exit_status) => {
                        if !exit_status.success() {
                            return Err(format!("Session ended with code: {}", exit_status).into());
                        }
                        let output = Output::new(opts.verbose);
                        output.stderr("Session ended");
                        return Ok(output);
                    }
                    Err(e) => {
                        eprintln!("Failed to wait for session process: {}", e);
                        process::exit(1);
                    }
                }
            }
            Err(err) => {
                use aws_sdk_ssm::error::SdkError;
                let error_msg = match err {
                    SdkError::ServiceError(service_err) => {
                        handle_service_error(service_err.err(), "SSM session")
                    }
                    SdkError::DispatchFailure(dispatch_err) => {
                        handle_dispatch_failure(&dispatch_err, "SSM session")
                    }
                    SdkError::ConstructionFailure(err) => {
                        handle_construction_failure(&err, "SSM session", false)
                    }
                    SdkError::TimeoutError(err) => handle_timeout_error(&err, "SSM session", false),
                    SdkError::ResponseError(err) => {
                        handle_response_error(&err, "SSM session", false)
                    }
                    _ => handle_unknown_error(&err, "SSM session", false),
                };
                Err(error_msg.into())
            }
        }
    }
}

/// Check if session-manager-plugin is installed
fn is_plugin_installed() -> bool {
    Command::new("which")
        .arg("session-manager-plugin")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}
