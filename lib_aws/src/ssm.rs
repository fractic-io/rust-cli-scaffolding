use aws_sdk_ssm::Client;
use lib_core::{define_cli_error, CliError};

use crate::shared_config::config_from_profile;

define_cli_error!(SsmError, "Error running AWS SSM command.");
define_cli_error!(SsmParameterNotFound, "No SSM parameter found with name: {name}.", { name: &str });

pub async fn get_ssm_parameter(
    profile: &str,
    region: &str,
    name: &str,
) -> Result<String, CliError> {
    let client = Client::new(&config_from_profile(profile, region).await);
    Ok(client
        .get_parameter()
        .name(name)
        .send()
        .await
        .map_err(|e| SsmError::with_debug(&e))?
        .parameter
        .ok_or_else(|| SsmParameterNotFound::new(name))?
        .value
        .as_ref()
        .ok_or_else(|| SsmParameterNotFound::new(name))?
        .clone())
}
