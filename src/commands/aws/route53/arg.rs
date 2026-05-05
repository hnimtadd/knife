use clap::{Args, Subcommand};

#[derive(Debug, Args)]
pub struct AWSRoute53Command {
    #[command(subcommand)]
    pub command: Route53SubCommand,
}

#[derive(Debug, Subcommand)]
pub enum Route53SubCommand {
    /// Get domain record configuration
    Get(GetArg),
}

#[derive(Debug, Args)]
pub struct GetArg {
    /// DNS name
    #[arg(long)]
    pub dns_name: String,
}
