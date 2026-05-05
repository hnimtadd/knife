use clap::{Args, Subcommand};

use crate::commands::utils;

#[derive(Debug, Args)]
pub struct AWSSSMCommand {
    #[command(subcommand)]
    pub command: SSMSubCommand,
}

#[derive(Debug, Subcommand)]
pub enum SSMSubCommand {
    /// start SSM session
    Start(StartArg),
}

#[derive(Debug, Args)]
pub struct StartArg {
    /// Arn
    #[arg(long)]
    #[clap(value_parser = utils::token_or_stdin_parser, default_value = "-")]
    pub id: String,
}
