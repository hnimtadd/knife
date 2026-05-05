use std::io::{self, Read};

use chrono::DateTime;

pub fn token_or_stdin_parser(s: &str) -> Result<String, String> {
    if s == "-" {
        let mut buffer_bytes = Vec::new();
        io::stdin()
            .read_to_end(&mut buffer_bytes)
            .map_err(|e| format!("Failed to read from stdin: {}", e))?;
        if buffer_bytes.is_empty() {
            Err("Empty token received from stdin".to_string())
        } else {
            let buffer = String::from_utf8(buffer_bytes)
                .map_err(|e| format!("Failed to decode stdin as UTF-8: {}", e))?;
            Ok(buffer.trim().to_string())
        }
    } else {
        Ok(s.to_string())
    }
}

pub fn parse_duration_to_seconds(val: &str) -> Result<i64, String> {
    use chrono::Utc;
    let now = Utc::now().timestamp();

    // Try to parse as absolute Unix timestamp first
    if let Ok(timestamp) = val.parse::<i64>() {
        // If it's a reasonable timestamp (after year 2000), use as absolute
        if timestamp > 946684800 {
            // Jan 1, 2000
            return Ok(timestamp);
        } else {
            // Otherwise treat as offset from now
            return Ok(now + timestamp);
        }
    }

    // Try to parse as various datetime formats
    if let Ok(timestamp) = DateTime::parse_from_rfc3339(val) {
        return Ok(timestamp.timestamp());
    }

    // Parse duration strings like "+1h", "-30m", "2h", etc.
    if val.starts_with('+') || val.starts_with('-') {
        let duration_str = &val[1..]; // Remove + or - prefix
        let is_positive = val.starts_with('+');

        match parse_duration::parse(duration_str) {
            Ok(duration) => {
                let offset = duration.as_secs() as i64;
                let final_timestamp = if is_positive {
                    now + offset
                } else {
                    now - offset
                };
                Ok(final_timestamp)
            }
            Err(e) => Err(format!("Failed to parse duration '{}': {}", val, e)),
        }
    } else {
        // Try parsing as plain duration string like "1h", "30m" (treat as positive offset)
        match parse_duration::parse(val) {
            Ok(duration) => Ok(now + (duration.as_secs() as i64)),
            Err(_) => Err(format!(
                "Failed to parse '{}'. Supported formats:\n\
                 • Unix timestamps: 1672531200\n\
                 • RFC3339: 2025-09-28T18:27:21Z\n\
                 • ISO dates: 2025-09-28, 2025-09-28 18:27:21\n\
                 • Durations: +1h, -30m, 2h30m",
                val
            )),
        }
    }
}
