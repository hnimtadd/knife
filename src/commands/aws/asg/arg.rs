use clap::{Args, Subcommand};

#[derive(Debug, Args)]
pub struct AWSASGCommand {
    /// ASG name
    #[arg(long)]
    pub name: String,

    #[command(subcommand)]
    pub command: ASGSubCommand,
}

#[derive(Debug, Subcommand)]
pub enum ASGSubCommand {
    /// Get ASG configuration
    Get,
    /// Scale ASG
    Scale(ScaleArg),
    /// Detach instances
    DetachInstances(DetachInstancesArg),
    /// Attach instances
    AttachInstances(AttachInstancesArg),
}

#[derive(Debug, Args)]
pub struct ScaleArg {
    /// Min size
    #[arg(long = "min")]
    pub min_size: Option<i32>,

    /// Max size
    #[arg(long = "max")]
    pub max_size: Option<i32>,

    /// Desired capacity
    #[arg(long = "desired")]
    pub desired_capacity: Option<i32>,

    /// Skip confirmation prompt
    #[arg(long)]
    pub yes: bool,
}

#[derive(Debug, Args)]
pub struct DetachInstancesArg {
    /// Instances ID
    #[arg(long,  num_args = 1..)]
    pub ids: Vec<String>,

    /// replace the detached instance with a new instance to maintain the group capacity.
    #[arg(long)]
    pub replace: bool,

    /// Skip confirmation prompt
    #[arg(long)]
    pub yes: bool,
}

#[derive(Debug, Args)]
pub struct AttachInstancesArg {
    /// Instances ID
    #[arg(long,  num_args = 1..)]
    pub ids: Vec<String>,
}
