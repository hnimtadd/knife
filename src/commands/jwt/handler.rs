use chrono::Utc;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashSet;

use crate::commands::{
    CommandHandler, JWTCommand, Output,
    jwt::arg::{JWTDecodeArgs, JWTEncodeArgs, JWTSubCommand, SupportedAlgorithms},
};

pub struct JWTHandler {
    cmd: JWTCommand,
}

impl JWTHandler {
    pub fn new(cmd: JWTCommand) -> Self {
        Self { cmd }
    }
}

impl CommandHandler for JWTHandler {
    async fn execute(self) -> Result<Output, Box<dyn std::error::Error>> {
        match self.cmd.command {
            JWTSubCommand::Encode(cmd) => handle_encode(cmd).map_err(|e| e.into()),
            JWTSubCommand::Decode(cmd) => handle_decode(cmd),
        }
    }
}
#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    #[serde(flatten)]
    extra: Map<String, Value>,
}
impl Claims {
    fn new() -> Self {
        Self { extra: Map::new() }
    }
}

fn handle_decode(cmd: JWTDecodeArgs) -> Result<Output, Box<dyn std::error::Error>> {
    if cmd.insecure {
        // Unverified parsing - ignore signature validation
        let mut validation = Validation::default();
        validation.insecure_disable_signature_validation();
        validation.validate_exp = false;
        validation.validate_nbf = false;
        validation.validate_aud = false;
        // Clear required claims - don't require any claims to be present
        validation.required_spec_claims = HashSet::new();

        // Use a dummy key since we're not validating signature
        let key = DecodingKey::from_secret(&[]);
        let output = Output::new(false);

        match decode::<Claims>(&cmd.token, &key, &validation) {
            Ok(token_data) => {
                output.stderr("JWT decoded successfully");
                let response = serde_json::json!({
                    "Header": token_data.header,
                    "Claims": token_data.claims.extra
                });
                output.stdout(&serde_json::to_string_pretty(&response).unwrap());
                Ok(output)
            }
            Err(e) => {
                output.stderr(&format!("Failed to decode JWT: {}", e));
                Err(Box::new(e))
            }
        }
    } else {
        // For verified decoding, we would need the secret/key, but the current args don't include it
        // This is a limitation of the current argument structure
        Err("Verified JWT decoding is not yet implemented. Use --insecure flag for token inspection.".to_string().into())
    }
}
fn create_header(alg: Algorithm, kid: Option<&String>, no_typ: bool) -> Header {
    let mut header = Header::new(alg);
    if let Some(k) = kid {
        header.kid = Some(k.clone());
    }
    if no_typ {
        header.typ = None;
    }
    header
}

fn handle_encode(cmd: JWTEncodeArgs) -> Result<Output, String> {
    let alg = translate_algorithm(&cmd.algorithm);
    let header = create_header(alg, cmd.kid.as_ref(), cmd.no_typ);

    // Build claims map
    let mut claims_map = Claims::new();

    // Add custom JSON payload if provided
    if let Some(ref json_str) = cmd.json {
        match serde_json::from_str::<Value>(json_str) {
            Ok(Value::Object(json_obj)) => {
                for (key, value) in json_obj {
                    claims_map.extra.insert(key, value);
                }
            }
            Ok(_) => return Err("JSON payload must be an object".into()),
            Err(e) => return Err(format!("Invalid JSON: {}", e).into()),
        }
    }

    let now = Utc::now().timestamp();

    // Add standard claims
    if !cmd.no_iat {
        claims_map
            .extra
            .insert("iat".to_string(), Value::Number(now.into()));
    }

    if let Some(exp_timestamp) = cmd.expires {
        claims_map
            .extra
            .insert("exp".to_string(), Value::Number(exp_timestamp.into()));
    }

    if let Some(ref iss) = cmd.issuer {
        claims_map
            .extra
            .insert("iss".to_string(), Value::String(iss.clone()));
    }

    if let Some(ref sub) = cmd.subject {
        claims_map
            .extra
            .insert("sub".to_string(), Value::String(sub.clone()));
    }

    if let Some(ref aud) = cmd.audience {
        claims_map
            .extra
            .insert("aud".to_string(), Value::String(aud.clone()));
    }

    if let Some(ref jti) = cmd.jwt_id {
        claims_map
            .extra
            .insert("jti".to_string(), Value::String(jti.clone()));
    }

    if let Some(nbf_timestamp) = cmd.not_before {
        claims_map
            .extra
            .insert("nbf".to_string(), Value::Number(nbf_timestamp.into()));
    }

    // Create encoding key based on algorithm type
    let encoding_key = match cmd.algorithm {
        SupportedAlgorithms::HS256 | SupportedAlgorithms::HS384 | SupportedAlgorithms::HS512 => {
            // HMAC algorithms use secrets
            EncodingKey::from_secret(cmd.secret.as_bytes())
        }
        SupportedAlgorithms::RS256
        | SupportedAlgorithms::RS384
        | SupportedAlgorithms::RS512
        | SupportedAlgorithms::PS256
        | SupportedAlgorithms::PS384
        | SupportedAlgorithms::PS512 => {
            // RSA algorithms expect PEM-encoded private keys
            match EncodingKey::from_rsa_pem(cmd.secret.as_bytes()) {
                Ok(key) => key,
                Err(e) => {
                    return Err(format!(
                        "Invalid RSA private key: {}. RSA algorithms require PEM-encoded private keys.",
                        e
                    ).into());
                }
            }
        }
        SupportedAlgorithms::ES256 | SupportedAlgorithms::ES384 => {
            // ECDSA algorithms expect PEM-encoded private keys
            match EncodingKey::from_ec_pem(cmd.secret.as_bytes()) {
                Ok(key) => key,
                Err(e) => {
                    return Err(format!(
                        "Invalid ECDSA private key: {}. ECDSA algorithms require PEM-encoded private keys.",
                        e
                    ).into());
                }
            }
        }
    };

    // Encode JWT
    match encode(&header, &claims_map, &encoding_key) {
        Ok(token) => {
            let output = Output::new(false);
            output.stderr("JWT encoded successfully");
            let response = serde_json::json!({
                "token": token
            });
            output.stdout(&serde_json::to_string_pretty(&response).unwrap());
            Ok(output)
        }
        Err(e) => Err(format!("Failed to encode JWT: {}", e).into()),
    }
}
pub fn translate_algorithm(alg: &SupportedAlgorithms) -> Algorithm {
    match alg {
        SupportedAlgorithms::HS256 => Algorithm::HS256,
        SupportedAlgorithms::HS384 => Algorithm::HS384,
        SupportedAlgorithms::HS512 => Algorithm::HS512,
        SupportedAlgorithms::RS256 => Algorithm::RS256,
        SupportedAlgorithms::RS384 => Algorithm::RS384,
        SupportedAlgorithms::RS512 => Algorithm::RS512,
        SupportedAlgorithms::PS256 => Algorithm::PS256,
        SupportedAlgorithms::PS384 => Algorithm::PS384,
        SupportedAlgorithms::PS512 => Algorithm::PS512,
        SupportedAlgorithms::ES256 => Algorithm::ES256,
        SupportedAlgorithms::ES384 => Algorithm::ES384,
    }
}
