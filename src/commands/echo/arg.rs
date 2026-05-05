use clap::Args;

#[derive(Debug, Args)]
pub struct EchoCommand {
    /// Address to listen on
    #[arg(short, long, default_value = "127.0.0.1:8080")]
    pub listen: String,

    /// Text to respond with
    #[arg(short, long, default_value = "OK")]
    pub text: String,

    /// Enable debug mode - dumps full HTTP requests
    #[arg(short, long)]
    pub debug: bool,
}
