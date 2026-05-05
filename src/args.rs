use crate::{
    commands::{AWSCommand, Base64Command, EchoCommand, JWTCommand},
    shells::Shell,
};
use clap::{Args, CommandFactory, Parser, Subcommand};
use clap_complete::generate;
use std::process;

#[derive(Debug, Parser)]
#[clap(author, version, about)]
pub struct KnifeArgs {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Echo command - starts echo services
    Echo(EchoCommand),
    /// JWT command - works with JWT
    Jwt(JWTCommand),
    /// Base64 command - works with base64
    Base64(Base64Command),
    /// AWS command - works with aws components
    Aws(AWSCommand),
    /// Activate handle setups knife related functionality for shell.
    Completion(CompletionCommand),
}

#[derive(Debug, Args)]
pub struct CompletionCommand {
    #[arg(value_enum)]
    shell: Option<Shell>,
    #[arg(short)]
    verbose: bool,
}

impl CompletionCommand {
    pub fn execute(&self) {
        let user_shell = if let Some(shell) = self.shell {
            shell
        } else {
            if self.verbose {
                // Auto-detect from SHELL environment variable
                eprintln!("shell not inputed, try to get from env");
            }
            self.detect_shell()
        };

        let mut cmd = KnifeArgs::command();
        let shell = match user_shell {
            Shell::Bash => clap_complete::Shell::Bash,
            Shell::Zsh => clap_complete::Shell::Zsh,
            Shell::Fish => clap_complete::Shell::Fish,
        };
        if self.verbose {
            eprintln!("try to generate completion scripts for {shell}");
        }

        generate(shell, &mut cmd, "knife", &mut std::io::stdout());
    }

    fn detect_shell(&self) -> Shell {
        let shell_path = match std::env::var("SHELL") {
            Ok(path) => path,
            Err(_) => {
                eprintln!("SHELL environment variable not found");
                eprintln!("Usage: knife activate --shell <bash|zsh|fish>");
                process::exit(1);
            }
        };

        let shell_name = shell_path.rsplit('/').next().unwrap_or(&shell_path);

        match shell_name {
            "zsh" => Shell::Zsh,
            "bash" => Shell::Bash,
            "fish" => Shell::Fish,
            _ => {
                eprintln!("Unsupported shell: {}", shell_name);
                eprintln!("Supported shells: bash, zsh, fish");
                eprintln!("Usage: knife activate --shell <bash|zsh|fish>");
                process::exit(1);
            }
        }
    }
}
