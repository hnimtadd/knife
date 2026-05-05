mod args;
mod commands;
mod shells;

use args::{Command, KnifeArgs};
use clap::Parser;

use crate::commands::{AWSHandler, Base64Handler, CommandHandler, EchoHandler, JWTHandler};

#[tokio::main]
async fn main() {
    #[cfg(unix)]
    unsafe {
        // Reset SIGPIPE to default behavior to avoid panic on broken pipe
        // NOTE, This is expected behavior.
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    let args = KnifeArgs::parse();

    match args.command {
        Command::Completion(activate_cmd) => {
            activate_cmd.execute();
        }
        Command::Echo(echo_cmd) => {
            let echo_handler = EchoHandler::new(echo_cmd);
            let _ = echo_handler.execute().await;
        }
        Command::Jwt(jwt_cmd) => {
            let jwt_handler = JWTHandler::new(jwt_cmd);
            match jwt_handler.execute().await {
                Ok(_output) => {
                    std::process::exit(0);
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Command::Base64(base64_cmd) => {
            let base64_handler = Base64Handler::new(base64_cmd);
            match base64_handler.execute().await {
                Ok(_output) => {
                    std::process::exit(0);
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Command::Aws(aws_cmd) => {
            let aws_handler = AWSHandler::new(aws_cmd);
            match aws_handler.execute().await {
                Ok(_output) => {
                    std::process::exit(0);
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}
