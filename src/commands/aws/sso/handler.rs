use dialoguer::FuzzySelect;
use ini::ini;
use std::fs;
use std::path::PathBuf;
use std::process::{self, Command};

use crate::commands::Output;
use crate::commands::aws::sso::arg::{AWSSSOCommand, LoginArgs, LogoutArgs, SSOSubCommand};
use crate::shells;

impl AWSSSOCommand {
    pub async fn execute(self, verbose: bool) -> Result<Output, Box<dyn std::error::Error>> {
        match self.command {
            SSOSubCommand::Login(args) => args.execute(verbose).await,
            SSOSubCommand::Logout(args) => args.execute(verbose).await,
        }
    }
}

impl LogoutArgs {
    async fn execute(self, verbose: bool) -> Result<Output, Box<dyn std::error::Error>> {
        let output = Output::new(verbose);

        // Perform SSO logout
        match sso_logout(self.profile.as_deref()).await {
            Ok(_) => {
                output.stderr("SSO logout completed successfully");
                Ok(output)
            }
            Err(e) => {
                output.stderr(&format!("SSO logout failed: {}", e));
                Err(e)
            }
        }
    }
}

impl LoginArgs {
    async fn execute(self, verbose: bool) -> Result<Output, Box<dyn std::error::Error>> {
        let output = Output::new(verbose);
        let mut login_url: Option<String> = None;

        // If profile is provided, use it; otherwise show interactive selection
        let profile = if let Some(p) = self.profile {
            p
        } else {
            // Get SSO profiles from ~/.aws/config
            let mut profiles = get_sso_profiles()?;

            if profiles.is_empty() {
                output.stderr("No AWS SSO profiles found in ~/.aws/config");
                return Err(format!("Error: No AWS SSO profiles found in ~/.aws/config").into());
            }

            // Set up Ctrl-C handler to restore terminal
            // Known issue that haven't solved at upstream.
            // dialoguer try to hide cursor while start the selection, but didn't
            // add any interrupt intercenption.
            // https://github.com/console-rs/dialoguer/issues/77
            if let Err(e) = ctrlc::set_handler(move || {
                // Restore terminal state
                let term = console::Term::stdout();
                let _ = term.show_cursor();
            }) {
                output.stderr(&format!("failed to setup ctrl_c handler. {e:?}"));
            };

            profiles.sort_by_key(|p| p.name.clone());
            // Interactive selection
            let profile_names: Vec<&str> = profiles.iter().map(|f| f.name.as_str()).collect();
            let selection = FuzzySelect::new()
                .with_prompt("Select AWS SSO profile")
                .items(&profile_names)
                .default(0)
                .interact_opt();

            match selection {
                Ok(Some(idx)) => {
                    let profile = &profiles[idx];
                    login_url = Some(format!(
                        "{}#/console?account_id={}&role_name={}",
                        profile.start_url, profile.account_id, profile.role_name
                    ));
                    profiles[idx].name.clone()
                }

                Ok(None) | Err(_) => {
                    // Restore cursor visibility on exit
                    output.stderr("\nSelection cancelled");
                    process::exit(1);
                }
            }
        };

        if verbose {
            output.stderr(&format!("Logging in with profile: {}", profile));
        }

        // Perform SSO login
        match sso_login(&profile, verbose).await {
            Ok(_) => {
                output.stderr("SSO login completed successfully");
                let response = serde_json::json!({
                    "profile": profile,
                    "_url": login_url,
                });
                output.stdout(&serde_json::to_string_pretty(&response).unwrap());
                Ok(output)
            }
            Err(e) => {
                output.stderr(&format!("SSO login failed: {}", e));
                Err(e)
            }
        }
    }
}

struct SSOProfile {
    pub name: String,
    pub start_url: String,
    pub account_id: String,
    pub role_name: String,
}

/// Get all SSO profiles from ~/.aws/config using INI parser
/// Returns profiles that have sso_start_url or sso_session configured
fn get_sso_profiles() -> Result<Vec<SSOProfile>, Box<dyn std::error::Error>> {
    // Get home directory and build config path
    let home = match std::env::var("HOME") {
        Ok(h) => h,
        Err(_) => {
            return Err(format!("HOME environment variable not set").into());
        }
    };

    let config_path = format!("{}/.aws/config", home);

    // Check if the config file exists
    if !std::path::Path::new(&config_path).exists() {
        return Err(format!("AWS config file not found: {}", config_path).into());
    }

    // Load and parse the config file
    let config = ini!(&config_path);

    let mut sso_profiles = Vec::<SSOProfile>::new();

    for (section, properties) in config.iter() {
        // Check if this section has SSO configuration
        let has_sso =
            properties.contains_key("sso_start_url") || properties.contains_key("sso_session");

        if has_sso {
            // Try to get start_url directly, or from sso_session reference
            let start_url = if let Some(Some(url)) = properties.get("sso_start_url") {
                url.to_string()
            } else if let Some(Some(sso_session_name)) = properties.get("sso_session") {
                // Look up the sso-session section to get the start URL
                let sso_section_name = format!("sso-session {}", sso_session_name);
                if let Some(sso_section) = config.get(&sso_section_name) {
                    if let Some(Some(sso_url)) = sso_section.get("sso_start_url") {
                        sso_url.to_string()
                    } else {
                        "unknown".to_string()
                    }
                } else {
                    "unknown".to_string()
                }
            } else {
                "unknown".to_string()
            };

            let role_name = if let Some(Some(role)) = properties.get("sso_role_name") {
                role.to_string()
            } else {
                "unknown".to_string()
            };

            let account_id = if let Some(Some(id)) = properties.get("sso_account_id") {
                id.to_string()
            } else {
                "unknown".to_string()
            };

            // Extract profile name from section name
            // Sections are either "default" or "profile <name>"
            let profile_name = if section == "default" {
                "default".to_string()
            } else if let Some(name) = section.strip_prefix("profile ") {
                name.to_string()
            } else {
                continue;
            };

            sso_profiles.push(SSOProfile {
                name: profile_name,
                start_url,
                account_id,
                role_name,
            });
        }
    }

    // sso_profiles.sort();
    Ok(sso_profiles)
}

async fn sso_logout(profile: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    // Execute aws sso login command
    let mut cmd = Command::new("aws");
    cmd.arg("sso").arg("logout");
    if let Some(p) = profile {
        cmd.arg("--profile").arg(p);
    };
    match cmd.status() {
        Ok(exit_status) => {
            if !exit_status.success() {
                return Err(format!("SSO logout failed with code: {}", exit_status).into());
            }

            // Save the profile and region for future knife commands
            if let Err(e) = clean_last_session() {
                return Err(format!("Warning: Failed to save session preferences: {}", e).into());
            }

            Ok(())
        }
        Err(e) => Err(format!("Failed to login SSO: {}", e).into()),
    }
}

async fn sso_login(profile: &str, verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    // Execute aws sso login command
    match Command::new("aws")
        .arg("sso")
        .arg("login")
        .arg("--profile")
        .arg(profile)
        .status()
    {
        Ok(exit_status) => {
            if !exit_status.success() {
                return Err(format!("SSO login failed with code: {}", exit_status).into());
            }

            // Detect the region from profile config
            let region = get_profile_region(profile, verbose);

            // Save the profile and region for future knife commands
            if let Err(e) = save_last_session(profile, region.as_deref()) {
                return Err(format!("Warning: Failed to save session preferences: {}", e).into());
            }

            Ok(())
        }
        Err(e) => Err(format!("Failed to login SSO: {}", e).into()),
    }
}

/// Get the knife config directory path (~/.knife)
fn get_config_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let home = std::env::var("HOME").map_err(|_| -> Box<dyn std::error::Error> {
        format!("HOME environment variable not set").into()
    })?;

    let config_dir = PathBuf::from(home).join(".knife");

    // Create directory if it doesn't exist
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir).map_err(|e| -> Box<dyn std::error::Error> {
            format!("Failed to create config directory: {}", e).into()
        })?;
    }

    Ok(config_dir)
}

fn clean_last_session() -> Result<(), Box<dyn std::error::Error>> {
    let config_dir = get_config_dir().map_err(|e| -> Box<dyn std::error::Error> {
        format!("failed to get config directory: {}", e).into()
    })?;
    let shell_config_dir = config_dir.join("shell");
    if shell_config_dir.is_dir() {
        for shell_config in fs::read_dir(shell_config_dir)? {
            let shell_config = shell_config?;
            let path = shell_config.path();
            if path.is_file() {
                fs::remove_file(path)?;
            };
        }
    };
    Ok(())
}

/// Save the last used AWS profile and region to knife config
fn save_last_session(
    profile: &str,
    region: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let config_dir = get_config_dir().map_err(|e| -> Box<dyn std::error::Error> {
        format!("Failed to get config directory: {}", e).into()
    })?;
    let config_file = config_dir.join("knife.toml");

    let mut content = format!("[aws]\nprofile = \"{}\"\n", profile);
    if let Some(r) = region {
        content.push_str(&format!("region = \"{}\"\n", r));
    }

    fs::write(config_file, content).map_err(|e| -> Box<dyn std::error::Error> {
        format!("Failed to save last session: {}", e).into()
    })?;

    // Write AWS_PROFILE and AWS_REGION to ~/.knife/shell/shell.zsh|shell.bash|shell.fish
    for shell in shells::Shell::keys() {
        let shell_config = match *shell {
            shells::Shell::Bash | shells::Shell::Zsh => {
                let mut shell_config = format!("export AWS_PROFILE=\"{}\"\n", profile);
                if let Some(r) = region {
                    shell_config.push_str(&format!("export AWS_REGION=\"{}\"\n", r));
                }
                shell_config
            }
            shells::Shell::Fish => {
                let mut shell_config = format!("set -x AWS_PROFILE \"{}\"\n", profile);
                if let Some(r) = region {
                    shell_config.push_str(&format!("set -x AWS_REGION \"{}\"\n", r))
                }
                shell_config
            }
        };
        let shell_config_dir = config_dir.join("shell");
        // Create directory if it doesn't exist
        if !shell_config_dir.exists() {
            fs::create_dir_all(&shell_config_dir).map_err(|e| -> Box<dyn std::error::Error> {
                format!("Failed to create shell config directory: {}", e).into()
            })?;
        };

        fs::write(
            shell_config_dir.join(format!("shell.{}", shell.name())),
            shell_config,
        )
        .map_err(|e| -> Box<dyn std::error::Error> {
            format!("Failed to save shell config: {}", e).into()
        })?;
    }

    Ok(())
}

/// Load the last used AWS profile from knife config
pub fn load_last_profile() -> Option<String> {
    load_last_session().map(|(profile, _)| profile)
}

/// Load the last used AWS region from knife config
pub fn load_last_region() -> Option<String> {
    load_last_session().and_then(|(_, region)| region)
}

/// Load the last used AWS session (profile and region) from knife config
pub fn load_last_session() -> Option<(String, Option<String>)> {
    let config_dir = get_config_dir().ok()?;
    let config_file = config_dir.join("knife.toml");

    if !config_file.exists() {
        return None;
    }

    let content = fs::read_to_string(config_file).ok()?;
    let config: toml::Value = toml::from_str(&content).ok()?;

    let aws_section = config.get("aws")?;
    let profile = aws_section.get("profile")?.as_str()?.to_string();
    let region = aws_section
        .get("region")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Some((profile, region))
}

/// Get the configured region for a specific AWS profile from ~/.aws/config
fn get_profile_region(profile_name: &str, verbose: bool) -> Option<String> {
    if verbose {
        eprintln!("\nGetting profile region")
    }
    let home = std::env::var("HOME").ok()?;
    let config_path = format!("{}/.aws/config", home);

    if !std::path::Path::new(&config_path).exists() {
        if verbose {
            eprintln!("AWS config file not found: {}", config_path);
        }
        return None;
    }

    let config = ini!(&config_path);

    // Section name is either "default" or "profile <name>"
    let section_name = if profile_name == "default" {
        "default"
    } else {
        &format!("profile {}", profile_name)
    };

    let section = config.get(section_name)?;
    if verbose {
        eprintln!("Profile section found for {}", section_name);
    }

    // First, try to get region directly from profile
    if let Some(Some(region)) = section.get("region") {
        if verbose {
            eprintln!("Found region directly from profile: {}", region.to_string());
        }
        return Some(region.to_string());
    }
    if verbose {
        eprintln!("Profile has no region, try to search from session config");
    }

    // If no region, try to get it from sso-session
    if let Some(Some(sso_session_name)) = section.get("sso_session") {
        if verbose {
            eprintln!("Found sso_session: {}", sso_session_name);
        }
        let sso_section_name = format!("sso-session {}", sso_session_name);
        if let Some(sso_section) = config.get(&sso_section_name) {
            if verbose {
                eprintln!("Found session config");
            }
            if let Some(Some(sso_region)) = sso_section.get("sso_region") {
                if verbose {
                    eprintln!("Found region from sso-session: {}", sso_region.to_string());
                }
                return Some(sso_region.to_string());
            }
        }
    }

    None
}
