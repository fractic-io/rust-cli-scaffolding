use std::collections::HashMap;

use aws_sdk_secretsmanager::Client;
use lib_core::{define_cli_error, CliError};

use crate::shared_config::config_from_profile;

define_cli_error!(
    AwsSecretsManagerError,
    "Error fetching secret '{secret_id}' from AWS Secrets Manager in region '{region}': {error}.",
    { secret_id: &str, region: &str, error: &str }
);

pub async fn get_secret(
    profile: &str,
    region: &str,
    secret_id: &str,
    key: &str,
) -> Result<String, CliError> {
    let client = Client::new(&config_from_profile(profile, region).await);

    // Fetch secrets JSON.
    let secrets_output = client
        .get_secret_value()
        .secret_id(secret_id)
        .send()
        .await
        .map_err(|e| {
            AwsSecretsManagerError::with_debug(secret_id, region, "AWS callout failed", &e)
        })?;
    let secrets_string = secrets_output.secret_string().ok_or_else(|| {
        AwsSecretsManagerError::new(secret_id, region, "could not parse secret value")
    })?;
    let secrets_json =
        serde_json::from_str::<HashMap<String, String>>(secrets_string).map_err(|e| {
            AwsSecretsManagerError::with_debug(secret_id, region, "could not parse JSON", &e)
        })?;

    // Fetch required keys from JSON.
    Ok(secrets_json.get(key).cloned().ok_or_else(|| {
        AwsSecretsManagerError::new(
            secret_id,
            region,
            &format!("secret did not contain key '{key}'"),
        )
    })?)
}
