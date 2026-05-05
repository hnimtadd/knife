use crate::commands::utils;
use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};

#[derive(Debug, Args)]
pub struct JWTCommand {
    #[command(subcommand)]
    pub command: JWTSubCommand,
}

#[derive(Debug, Subcommand)]
pub enum JWTSubCommand {
    /// Decode decodes a jwt token and get payload value
    Decode(JWTDecodeArgs),
    /// Encode encodes a payload and get the jwt token
    Encode(JWTEncodeArgs),
}

#[derive(Debug, Args)]
pub struct JWTDecodeArgs {
    /// Token to decode with, either token or stdin must be available
    #[clap(index = 1)]
    #[clap(value_parser = utils::token_or_stdin_parser, default_value = "-")]
    pub token: String,

    /// Unverified parsed the token
    #[arg(long)]
    pub insecure: bool,
}

#[derive(Debug, Args)]
pub struct JWTEncodeArgs {
    /// The algorithm to use for signing the JWT
    #[clap(long = "alg", short = 'A')]
    #[clap(value_enum)]
    #[clap(default_value = "HS256")]
    pub algorithm: SupportedAlgorithms,

    /// The kid to place in the header
    #[clap(long = "kid", short = 'k')]
    pub kid: Option<String>,

    /// JSON payload to encode (omit to read from stdin)
    #[clap(index = 1)]
    #[clap(value_parser = utils::token_or_stdin_parser, default_value = "-")]
    pub json: Option<String>,

    /// prevent typ from being added to the header
    #[clap(long)]
    #[clap(value_parser)]
    pub no_typ: bool,

    /// The secret to sign the JWT with
    #[clap(long, short = 'S')]
    pub secret: String,

    /// The time the token should expire
    /// Supports: Unix timestamps, RFC3339 (2025-09-28T18:27:21Z), ISO dates, duration (+1h, 30m)
    #[clap(long = "exp", short = 'e')]
    #[clap(num_args = 0..=1)]
    #[clap(require_equals = true)]
    #[clap(value_parser = utils::parse_duration_to_seconds)]
    #[clap(default_value = None)]
    #[clap(default_missing_value = "+30m")] // 30 minutes from now
    pub expires: Option<i64>,

    /// The issuer of the token
    #[clap(long = "iss", short = 'i')]
    pub issuer: Option<String>,

    /// The subject of the token
    #[clap(long = "sub", short = 's')]
    pub subject: Option<String>,

    /// The audience of the token
    #[clap(long = "aud", short = 'a')]
    pub audience: Option<String>,

    /// The JWT ID of the token
    #[clap(long = "jti")]
    pub jwt_id: Option<String>,

    /// The time the JWT should become valid
    /// Supports: Unix timestamps, RFC3339 (2025-09-28T18:27:21Z), ISO dates, duration (+1h, 30m)
    #[clap(long = "nbf", short = 'n')]
    #[clap(value_parser = utils::parse_duration_to_seconds)]
    pub not_before: Option<i64>,

    /// Prevent an iat claim from being automatically added
    #[clap(long)]
    pub no_iat: bool,

    /// The path of the file to write the result to
    #[clap(long = "out", short = 'o')]
    pub output_path: Option<PathBuf>,

    /// Keep payload claims in the order they were added
    #[clap(long)]
    pub keep_payload_order: bool,
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, PartialEq, Eq, ValueEnum)]
#[clap(rename_all = "UPPERCASE")]
pub enum SupportedAlgorithms {
    HS256,
    HS384,
    HS512,
    RS256,
    RS384,
    RS512,
    PS256,
    PS384,
    PS512,
    ES256,
    ES384,
}
