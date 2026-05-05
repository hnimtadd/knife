use aws_config::{BehaviorVersion, Region, SdkConfig};

use crate::commands::{
    AWSCommand, CommandHandler, Output,
    aws::{
        arg::{AWSSubCommand, GlobalOptions},
        sso::handler::{load_last_profile, load_last_region},
    },
};

pub struct AWSHandler {
    cmd: AWSCommand,
}

impl CommandHandler for AWSHandler {
    async fn execute(self) -> Result<Output, Box<dyn std::error::Error>> {
        let AWSCommand {
            command,
            region,
            profile,
            verbose,
        } = self.cmd;

        // Some commands don't need to load AWS config, just execute them directly
        if let AWSSubCommand::SSO(sso_cmd) = command {
            // SSO login doesn't require credentials
            let _ = sso_cmd.execute(verbose).await;
            let output = Output::new(verbose);
            output.stderr("SSO login completed");
            return Ok(output);
        }

        let sdk_config = Self::load_sdk_config(region, profile, verbose).await?;
        let opts = GlobalOptions {
            verbose,
            sdk_config,
        };

        // Note: SSO command is handled above before credential loading
        match command {
            AWSSubCommand::Elb(elb_cmd) => elb_cmd.execute(opts).await,
            AWSSubCommand::Whoami(whoami_cmd) => whoami_cmd.execute(opts).await,
            AWSSubCommand::Route53(route53_cmd) => route53_cmd.execute(opts).await,
            AWSSubCommand::EC2(ec2_cmd) => ec2_cmd.execute(opts).await,
            AWSSubCommand::ASG(asg_cmd) => asg_cmd.execute(opts).await,
            AWSSubCommand::SSM(ssm_cmd) => ssm_cmd.execute(opts).await,
            AWSSubCommand::SSO(_) => {
                unreachable!("SSO command should have been handled earlier")
            }
        }
    }
}

impl AWSHandler {
    pub fn new(cmd: AWSCommand) -> Self {
        AWSHandler { cmd }
    }

    async fn load_sdk_config(
        region: Option<String>,
        profile: Option<String>,
        verbose: bool,
    ) -> Result<SdkConfig, Box<dyn std::error::Error>> {
        // Start with AWS defaults (respects AWS_PROFILE, AWS_REGION, etc.)
        let mut config_loader = aws_config::defaults(BehaviorVersion::latest());
        // Profile selection priority:
        // 1. Explicit --profile flag
        // 2. AWS_PROFILE environment variable
        // 3. Last used profile (from knife config)
        // 4. Default profile
        let selected_profile = if let Some(profile_name) = &profile {
            if verbose {
                eprintln!("Using profile from --profile flag: {}", profile_name);
            }
            Some(profile_name.clone())
        } else if let Ok(env_profile) = std::env::var("AWS_PROFILE") {
            if verbose {
                eprintln!("Using profile from AWS_PROFILE env: {}", env_profile);
            }
            Some(env_profile)
        } else if let Some(last_profile) = load_last_profile() {
            if verbose {
                eprintln!(
                    "Using last used profile from knife config: {}",
                    last_profile
                );
            }
            Some(last_profile)
        } else {
            if verbose {
                eprintln!("No profile specified, using default AWS profile");
            }
            None
        };

        if let Some(profile_name) = selected_profile {
            config_loader = config_loader.profile_name(profile_name);
        }

        // Region selection priority:
        // 1. Explicit --region flag
        // 2. AWS_REGION environment variable
        // 3. Last used region (from knife config)
        // 4. Profile's configured region (AWS SDK handles this)
        let selected_region = if let Some(region_str) = region {
            if verbose {
                eprintln!("Using region from --region flag: {}", region_str);
            }
            Some(region_str)
        } else if let Ok(env_region) = std::env::var("AWS_REGION") {
            if verbose {
                eprintln!("Using region from AWS_REGION env: {}", env_region);
            }
            Some(env_region)
        } else if let Some(last_region) = load_last_region() {
            if verbose {
                eprintln!("Using last used region from knife config: {}", last_region);
            }
            Some(last_region)
        } else {
            if verbose {
                eprintln!("Using region from profile config");
            }
            None
        };

        if let Some(region_str) = selected_region {
            config_loader = config_loader.region(Region::new(region_str));
        }

        // Try to load the config - AWS SDK will validate profile exists
        let sdk_config = config_loader.load().await;

        // Verify credentials are available
        if sdk_config.credentials_provider().is_none() {
            return Err("No AWS credentials found\n\nPlease ensure:\n  1. Profile exists\n  2. AWS_PROFILE environment variable is set, or\n  3. Use --profile flag to specify a profile".into());
        }

        // Verify region is set
        if sdk_config.region().is_none() {
            return Err("Region not found\n\nPlease ensure:\n  1. Region is set in current profile\n  2. AWS_REGION environment variable is set, or\n  3. Use --region flag to specify a region".into());
        }
        Ok(sdk_config)
    }
}
