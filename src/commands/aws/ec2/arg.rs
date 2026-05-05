use clap::{Args, Subcommand};

#[derive(Debug, Args)]
pub struct AWSEC2Command {
    #[command(subcommand)]
    pub command: EC2SubCommand,
}

#[derive(Debug, Subcommand)]
pub enum EC2SubCommand {
    /// Search EC2 instances by name or IP
    Get(SearchArg),
    /// Terminate an EC2 instance (shutdown -> terminate flow)
    Terminate(TerminateArg),
}

#[derive(Debug, Args)]
pub struct SearchArg {
    /// Enable fuzzy matching for names
    #[arg(long)]
    pub fuzzy: bool,

    /// Search by private IP address
    #[arg(long, value_parser = validate_ip_address)]
    pub private_ip: Option<String>,

    /// Search by public IP address
    #[arg(long, value_parser = validate_ip_address)]
    pub public_ip: Option<String>,

    /// Search by instance ID
    #[arg(long = "id")]
    pub instance_id: Option<String>,

    /// Search by instance name (exact match)
    #[arg(long, value_parser = validate_name)]
    pub name: Option<String>,

    /// Filter by instance state (running, stopped, pending, etc.)
    /// Can be specified multiple times for multiple states
    #[arg(long, value_parser = validate_state, num_args = 1..)]
    pub state: Vec<String>,
}

fn validate_ip_address(s: &str) -> Result<String, String> {
    // Simple IP validation
    if s.contains('.') && s.split('.').count() == 4 {
        // Check if all parts are valid numbers
        for part in s.split('.') {
            if part.parse::<u8>().is_ok() {
                // u8 is already 0-255, so no need to check > 255
            } else {
                return Err(format!("Invalid IP address: {} (non-numeric octet)", s));
            }
        }
        Ok(s.to_string())
    } else {
        Err(format!(
            "Invalid IP address format: {} (expected format: x.x.x.x)",
            s
        ))
    }
}

fn validate_name(s: &str) -> Result<String, String> {
    if s.is_empty() {
        Err("Instance name cannot be empty".to_string())
    } else if s.len() > 255 {
        Err("Instance name too long (max 255 characters)".to_string())
    } else {
        Ok(s.to_string())
    }
}

fn validate_state(s: &str) -> Result<String, String> {
    let valid_states = [
        "pending",
        "running",
        "shutting-down",
        "terminated",
        "stopping",
        "stopped",
    ];
    let state_lower = s.to_lowercase();

    if valid_states.contains(&state_lower.as_str()) {
        Ok(state_lower)
    } else {
        Err(format!(
            "Invalid instance state: '{}'. Valid states are: {}",
            s,
            valid_states.join(", ")
        ))
    }
}

#[derive(Debug, Args)]
pub struct TerminateArg {
    /// Instance ID to terminate
    #[arg(long = "id", required = true)]
    pub instance_id: String,

    /// Skip confirmation prompt
    #[arg(long)]
    pub yes: bool,
}

impl SearchArg {
    pub fn validate(&self) -> Result<(), String> {
        // Check that at least one search parameter is provided
        if self.name.is_none()
            && self.private_ip.is_none()
            && self.public_ip.is_none()
            && self.state.is_empty()
            && self.instance_id.is_none()
        {
            return Err("At least one search parameter must be provided".to_string());
        }
        Ok(())
    }
}
