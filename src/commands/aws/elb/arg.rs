use clap::{Args, Subcommand};

use crate::commands::utils;
#[derive(Debug, Args)]
pub struct AWSElbCommand {
    #[command(subcommand)]
    pub command: ElbSubCommand,
}

#[derive(Debug, Subcommand)]
pub enum ElbSubCommand {
    /// Get command - Get load balacner using name
    Get(GetArg),
    /// get-listener - Get load balacner's listener using lb arn
    GetListeners(GetListenersArg),
    /// get-rules - Get listener's rules using listener_arn
    GetRules(GetRulesArg),
}

#[derive(Debug, Args)]
pub struct GetArg {
    /// Name of ALBs, knife will perform full text search on ALB names
    #[arg(long)]
    pub name: String,
    /// Number of records should be returned.
    #[arg(long)]
    pub num: Option<i8>,
    /// perform fuzzy search using the full name
    #[arg(long, default_value_t = true)]
    pub fuzzy: bool,
}

#[derive(Debug, Args)]
pub struct GetRulesArg {
    /// ARN of Listener which contains the rules
    #[arg(long = "arn")]
    #[clap(value_parser = utils::token_or_stdin_parser, default_value = "-")]
    pub listener_arn: String,

    /// Number of records should be returned.
    #[arg(long)]
    pub num: Option<i8>,

    /// Space seperated key value pair of tag that will be used to filter rule
    #[arg(long, value_delimiter = ' ', num_args = 2, value_names = ["KEY", "VALUE"])]
    pub tag: Option<Vec<String>>,
}

#[derive(Debug, Args)]
pub struct GetListenersArg {
    /// Load balancer ARN which contains this listener
    #[arg(long = "arn")]
    #[clap(value_parser = utils::token_or_stdin_parser, default_value = "-")]
    pub loadbalancer_arn: String,
}
