use std::collections::{HashMap, HashSet};

use aws_sdk_secretsmanager::{
    error::SdkError, operation::describe_secret::DescribeSecretError, types::ReplicaRegionType,
    Client,
};
use lib_core::{define_cli_error, CliError, Printer};

use crate::shared_config::config_from_profile;

define_cli_error!(
    FailedToFetchAwsSecret,
    "Error fetching secret '{secret_id}' from AWS Secrets Manager in region '{region}': {error}.",
    { secret_id: &str, region: &str, error: &str }
);

define_cli_error!(
    AwsSecretSubkeyNotFound,
    "Secret '{secret_id}' in region '{region}' does not contain subkey {subkey}.",
    { secret_id: &str, region: &str, subkey: &str }
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

pub async fn get_secret(profile: &str, region: &str, secret_id: &str) -> Result<String, CliError> {
    let client = Client::new(&config_from_profile(profile, region).await);
    match client.get_secret_value().secret_id(secret_id).send().await {
        Ok(output) => Ok(output
            .secret_string()
            .ok_or_else(|| {
                FailedToFetchAwsSecret::new(secret_id, region, "could not parse secret value")
            })?
            .to_owned()),
        Err(e) => Err(AwsSecretsManagerError::with_debug(&e)),
    }
}

pub async fn get_secret_subkey(
    profile: &str,
    region: &str,
    secret_id: &str,
    subkey: &str,
) -> Result<String, CliError> {
    // Fetch secrets JSON.
    let raw = get_secret(profile, region, secret_id).await?;
    let json = serde_json::from_str::<HashMap<String, String>>(&raw).map_err(|e| {
        FailedToFetchAwsSecret::with_debug(secret_id, region, "could not parse JSON", &e)
    })?;

    // Fetch required key from JSON.
    Ok(json
        .get(subkey)
        .cloned()
        .ok_or_else(|| AwsSecretSubkeyNotFound::new(secret_id, region, subkey))?)
}

pub async fn secret_replica_regions(
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

/// Returns true if new secret was created.
pub async fn update_or_create_secret(
    pr: &Printer,
    profile: &str,
    region: &str,
    secret_id: &str,
    value: &str,
    replica_regions: Option<&HashSet<String>>,
) -> Result<bool, CliError> {
    let client = Client::new(&config_from_profile(profile, region).await);
    match secret_exists(profile, region, secret_id).await? {
        true => {
            pr.info(&format!("Updating secret '{secret_id}'..."));
            client
                .put_secret_value()
                .secret_id(secret_id)
                .secret_string(value)
                .send()
                .await
                .map_err(|e| AwsSecretsManagerError::with_debug(&e))?;
            pr.info("Secret updated.");
            Ok(false)
        }
        false => {
            pr.info(&format!(
                "Creating secret '{secret_id}' with replication regions '{:?}'...",
                replica_regions,
            ));
            client
                .create_secret()
                .name(secret_id)
                .secret_string(value)
                .set_add_replica_regions(replica_regions.map(|r| {
                    r.into_iter()
                        .map(|r| ReplicaRegionType::builder().region(r).build())
                        .collect()
                }))
                .send()
                .await
                .map_err(|e| AwsSecretsManagerError::with_debug(&e))?;
            pr.info("Secret created.");
            Ok(true)
        }
    }
}
