use aws_sdk_autoscaling::Client;
use lib_core::{define_cli_error, CliError, Printer};

use crate::shared_config::config_from_profile;

define_cli_error!(Ec2Error, "Error running AWS EC2 command.");

pub async fn set_auto_scaling_group_desired_size(
    pr: &Printer,
    profile: &str,
    region: &str,
    auto_scaling_group: &str,
    desired_size: i32,
) -> Result<(), CliError> {
    pr.info(&format!(
        "Setting desired size of auto scaling group '{}' to {}...",
        auto_scaling_group, desired_size
    ));
    let client = Client::new(&config_from_profile(profile, region).await);
    client
        .update_auto_scaling_group()
        .auto_scaling_group_name(auto_scaling_group)
        .desired_capacity(desired_size)
        .send()
        .await
        .map_err(|e| Ec2Error::with_debug(&e))?;
    Ok(())
}
