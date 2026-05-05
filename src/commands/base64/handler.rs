use base64::prelude::*;

use crate::commands::{
    CommandHandler, Output,
    base64::arg::{Base64Command, Base64DecodeArgs, Base64EncodeArgs, Base64SubCommand},
};

pub struct Base64Handler {
    cmd: Base64Command,
}

impl Base64Handler {
    pub fn new(cmd: Base64Command) -> Self {
        Base64Handler { cmd }
    }
}

impl CommandHandler for Base64Handler {
    async fn execute(self) -> Result<Output, Box<dyn std::error::Error>> {
        match self.cmd.command {
            Base64SubCommand::Encode(encode_args) => {
                handle_encode(encode_args).map_err(|e| e.into())
            }
            Base64SubCommand::Decode(decode_args) => {
                handle_decode(decode_args).map_err(|e| e.into())
            }
        }
    }
}
fn handle_encode(args: Base64EncodeArgs) -> Result<Output, String> {
    let payload_len = args.payload.len();
    let encoded = BASE64_STANDARD.encode(args.payload);
    let output = Output::new(false);
    output.stderr(&format!("Encoded {} characters", payload_len));
    output.stdout(&encoded);
    Ok(output)
}
fn handle_decode(args: Base64DecodeArgs) -> Result<Output, String> {
    let result = BASE64_STANDARD.decode(args.token);
    match result {
        Ok(decoded) => {
            let decoded_len = decoded.len();
            let str = String::from_utf8(decoded).map_err(|e| format!("Decode error: {}", e))?;
            let output = Output::new(false);
            output.stderr(&format!("Decoded {} bytes", decoded_len));
            output.stdout(&str);
            Ok(output)
        }
        Err(err) => Err(format!("Failed to decode base64: {}", err)),
    }
}
