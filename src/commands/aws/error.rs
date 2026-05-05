use std::error::Error;
use std::fmt::Debug;

/// Check if the error is authentication-related
fn is_auth_error(error_msg: &str) -> bool {
    let lower = error_msg.to_lowercase();
    lower.contains("unauthorized")
        || lower.contains("access denied")
        || lower.contains("session token")
        || lower.contains("credentials")
        || lower.contains("authentication")
        || lower.contains("token expired")
        || lower.contains("invalid credentials")
}

/// Check if the error is permission-related
fn is_permission_error(error_msg: &str) -> bool {
    let lower = error_msg.to_lowercase();
    lower.contains("forbidden")
        || lower.contains("permission")
        || lower.contains("policy")
        || lower.contains("not authorized")
        || lower.contains("access denied")
}

/// Check if the error is a validation error
fn is_validation_error(error_msg: &str) -> bool {
    let lower = error_msg.to_lowercase();
    lower.contains("validation")
        || lower.contains("invalid")
        || lower.contains("minimum")
        || lower.contains("maximum")
        || lower.contains("desired capacity")
}

/// Handle AWS SDK ServiceError - extracts message, code, and request ID
pub fn handle_service_error<E>(err: &E, operation: &str) -> String
where
    E: Error + Debug,
{
    let error_msg = err.to_string();

    if is_auth_error(&error_msg) {
        format!(
            "Failed to {}: Authentication error\nError: {}\nHint: Your AWS credentials may have expired. Try refreshing your session or running 'aws sso login'.",
            operation, error_msg
        )
    } else if is_permission_error(&error_msg) {
        format!(
            "Failed to {}: Permission denied\nError: {}\nHint: You may not have the required IAM permissions. Check your IAM policy.",
            operation, error_msg
        )
    } else if is_validation_error(&error_msg) {
        let mut msg = format!(
            "Failed to {}: Validation error\nError: {}",
            operation, error_msg
        );
        // Add helpful hint for ASG detach operations
        if operation.contains("detach") {
            msg.push_str("\nHint: This error often occurs when detaching instances would cause the desired capacity to fall below the minimum size. Try using --replace flag to maintain capacity.");
        }
        msg
    } else {
        format!(
            "Failed to {}: Service error\nError: {}",
            operation, error_msg
        )
    }
}

/// Handle AWS SDK DispatchFailure - network/transport errors
pub fn handle_dispatch_failure<E>(err: &E, operation: &str) -> String
where
    E: Debug,
{
    let error_msg = format!("{:?}", err);

    if is_auth_error(&error_msg) {
        format!(
            "Failed to {}: Authentication error\nError: Session token not found or invalid\nHint: Your AWS credentials may have expired. Try refreshing your session or running 'aws sso login'.",
            operation
        )
    } else {
        format!(
            "Failed to {}: Network or connection error\nError: {}",
            operation, error_msg
        )
    }
}

/// Handle AWS SDK ConstructionFailure - request construction errors
pub fn handle_construction_failure<E>(err: &E, operation: &str, verbose: bool) -> String
where
    E: Debug,
{
    let mut error_msg = format!("Failed to {}: Invalid request", operation);
    if verbose {
        error_msg.push_str(&format!("\nDetails: {:?}", err));
    }
    error_msg
}

/// Handle AWS SDK TimeoutError - request timeout errors
pub fn handle_timeout_error<E>(err: &E, operation: &str, verbose: bool) -> String
where
    E: Debug,
{
    let mut error_msg = format!("Failed to {}: Request timed out", operation);
    if verbose {
        error_msg.push_str(&format!("\nDetails: {:?}", err));
    }
    error_msg
}

/// Handle AWS SDK ResponseError - response parsing errors
pub fn handle_response_error<E>(err: &E, operation: &str, verbose: bool) -> String
where
    E: Debug,
{
    let mut error_msg = format!("Failed to {}: Invalid response", operation);
    if verbose {
        error_msg.push_str(&format!("\nDetails: {:?}", err));
    }
    error_msg
}

/// Handle unknown AWS SDK errors
pub fn handle_unknown_error<E>(err: &E, operation: &str, verbose: bool) -> String
where
    E: std::fmt::Display + Debug,
{
    let mut error_msg = format!("Failed to {}: {}", operation, err);
    if verbose {
        error_msg.push_str(&format!("\nDetails: {:?}", err));
    }
    error_msg
}
