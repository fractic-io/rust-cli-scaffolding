use std::collections::{HashMap, HashSet};

use aws_sdk_secretsmanager::{
    error::SdkError, operation::describe_secret::DescribeSecretError, Client,
};
use lib_core::{define_cli_error, CliError};

use crate::shared_config::config_from_profile;

define_cli_error!(
    FailedToFetchAwsSecret,
    "Error fetching secret '{secret_id}' from AWS Secrets Manager in region '{region}': {error}.",
    { secret_id: &str, region: &str, error: &str }
);

define_cli_error!(
    AwsSecretsManagerError,
    "Error running AWS Secrets Manager command."
);

pub async fn secret_exists(profile: &str, region: &str, secret_id: &str) -> Result<bool, CliError> {
    let client = Client::new(&config_from_profile(profile, region).await);
    let response = client.describe_secret().secret_id(secret_id).send().await;
    match response {
        Ok(_) => Ok(true),
        Err(SdkError::<DescribeSecretError>::ServiceError(se))
            if se.err().is_resource_not_found_exception() =>
        {
            Ok(false)
        }
        Err(e) => Err(AwsSecretsManagerError::with_debug(&e)),
    }
}

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
        .map_err(|e| AwsSecretsManagerError::with_debug(&e))?;
    let secrets_string = secrets_output.secret_string().ok_or_else(|| {
        FailedToFetchAwsSecret::new(secret_id, region, "could not parse secret value")
    })?;
    let secrets_json =
        serde_json::from_str::<HashMap<String, String>>(secrets_string).map_err(|e| {
            FailedToFetchAwsSecret::with_debug(secret_id, region, "could not parse JSON", &e)
        })?;

    // Fetch required keys from JSON.
    Ok(secrets_json.get(key).cloned().ok_or_else(|| {
        FailedToFetchAwsSecret::new(
            secret_id,
            region,
            &format!("secret did not contain key '{key}'"),
        )
    })?)
}

pub async fn secret_replication_regions(
    profile: &str,
    region: &str,
    secret_id: &str,
) -> Result<HashSet<String>, CliError> {
    let client = Client::new(&config_from_profile(profile, region).await);
    let response = client
        .describe_secret()
        .secret_id(secret_id)
        .send()
        .await
        .map_err(|e| AwsSecretsManagerError::with_debug(&e))?;
    Ok(response
        .replication_status()
        .into_iter()
        .filter_map(|status| status.region())
        .map(|region| region.to_string())
        .collect())
}
