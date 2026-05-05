use crate::commands::utils;
use clap::{Args, Subcommand};

#[derive(Debug, Args)]
pub struct Base64Command {
    #[command(subcommand)]
    pub command: Base64SubCommand,
}

#[derive(Debug, Subcommand)]
pub enum Base64SubCommand {
    /// Decode decodes a base64 hash and the value
    Decode(Base64DecodeArgs),
    /// Encode encodes a value into the base64 hash
    Encode(Base64EncodeArgs),
}

#[derive(Debug, Args)]
pub struct Base64DecodeArgs {
    /// Token to decode with, either token or stdin must be available
    #[clap(index = 1)]
    #[clap(value_parser = utils::token_or_stdin_parser, default_value = "-")]
    pub token: String,
}

#[derive(Debug, Args)]
pub struct Base64EncodeArgs {
    /// payload to encode (omit to read from stdin)
    #[clap(index = 1)]
    #[clap(value_parser = utils::token_or_stdin_parser, default_value = "-")]
    pub payload: String,
}
