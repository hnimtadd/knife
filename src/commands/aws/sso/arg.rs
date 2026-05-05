use clap::{Args, Subcommand};

#[derive(Debug, Args)]
pub struct AWSSSOCommand {
    #[command(subcommand)]
    pub command: SSOSubCommand,
}

#[derive(Debug, Subcommand)]
pub enum SSOSubCommand {
    /// Login use sso session
    Login(LoginArgs),
    /// Logout current sso session
    Logout(LogoutArgs),
}

#[derive(Debug, Args)]
pub struct LogoutArgs {
    /// Profile name (if not provided, will logout all profile)
    #[arg(long)]
    pub profile: Option<String>,
}

#[derive(Debug, Args)]
pub struct LoginArgs {
    /// Profile name (if not provided, will show interactive selection)
    #[arg(long)]
    pub profile: Option<String>,
}
