use aws_config::SdkConfig;
use clap::{Args, Subcommand};

use crate::commands::aws::{
    asg::arg::AWSASGCommand, ec2::arg::AWSEC2Command, elb::arg::AWSElbCommand,
    route53::arg::AWSRoute53Command, ssm::arg::AWSSSMCommand, sso::arg::AWSSSOCommand,
    whoami::arg::AWSWhoAmICommand,
};
#[derive(Debug, Args)]
pub struct AWSCommand {
    /// the aws command that we work with
    #[command(subcommand)]
    pub command: AWSSubCommand,

    /// The AWS Region.
    #[clap(long, short)]
    pub region: Option<String>,

    /// The AWS Profile.
    #[clap(long, short)]
    pub profile: Option<String>,

    /// Whether to display additional information.
    #[clap(long, short)]
    pub verbose: bool,
}

#[derive(Debug, Subcommand)]
pub enum AWSSubCommand {
    /// work with elb related resources like loadbalancers, listeners, rules
    Elb(AWSElbCommand),
    /// unix whoami, but for AWS
    Whoami(AWSWhoAmICommand),
    /// work with route53 related resource like domain
    Route53(AWSRoute53Command),
    /// Work with ec2 related resource like instance
    EC2(AWSEC2Command),
    /// Work with auto scaling related resources like asg.
    ASG(AWSASGCommand),
    /// Work with service system manager.
    SSM(AWSSSMCommand),
    /// Work with sinngle sign on.
    SSO(AWSSSOCommand),
}

#[derive(Debug)]
pub struct GlobalOptions {
    pub sdk_config: SdkConfig,
    pub verbose: bool,
}
