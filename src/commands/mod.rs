pub mod aws;
pub mod base64;
pub mod echo;
pub mod jwt;
pub mod output;
pub mod utils;

pub use aws::arg::AWSCommand;
pub use base64::arg::Base64Command;
pub use echo::arg::EchoCommand;
pub use jwt::arg::JWTCommand;

pub use aws::handler::AWSHandler;
pub use base64::handler::Base64Handler;
pub use echo::handler::EchoHandler;
pub use jwt::handler::JWTHandler;

pub trait CommandHandler {
    async fn execute(self) -> Result<Output, Box<dyn std::error::Error>>;
}

// Re-export output types
pub use output::Output;
