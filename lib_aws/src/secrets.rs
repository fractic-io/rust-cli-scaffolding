use std::collections::{HashMap, HashSet};

use aws_sdk_secretsmanager::{
    error::SdkError, operation::describe_secret::DescribeSecretError, types::ReplicaRegionType,
    Client,
};
use lib_core::{define_cli_error, CliError, Printer};
use serde::de::DeserializeOwned;

use crate::shared_config::config_from_profile;

define_cli_error!(
    FailedToFetchAwsSecret,
    "Error fetching secret '{secret_id}' from AWS Secrets Manager in region '{region}': {error}.",
    { secret_id: &str, region: &str, error: &str }
);

define_cli_error!(
    AwsSecretSubkeysNotFound,
    "Secret '{secret_id}' in region '{region}' does not contain subkey(s) {missing_subkeys:?}.",
    { secret_id: &str, region: &str, missing_subkeys: &HashSet<String> }
);

define_cli_error!(
    AwsSecretsManagerError,
    "Error running AWS Secrets Manager command."
);

define_cli_error!(
    FailedToDeserializeAwsSecretSubkey,
    "Secret '{secret_id}' in region '{region}' contains an invalid value for subkey '{subkey}'.",
    { secret_id: &str, region: &str, subkey: &str }
);

// Public functions.
// ----------------------------------------------------------------------------

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

pub async fn get_secret_subkeys<T: DeserializeOwned>(
    profile: &str,
    region: &str,
    secret_id: &str,
    subkeys: HashSet<String>,
) -> Result<HashMap<String, T>, CliError> {
    let raw = get_secret(profile, region, secret_id).await?;
    parse_secret_subkeys(&raw, secret_id, region, &subkeys)
}

pub async fn get_secret_subkey<T: DeserializeOwned>(
    profile: &str,
    region: &str,
    secret_id: &str,
    subkey: &str,
) -> Result<T, CliError> {
    Ok(get_secret_subkeys::<T>(
        profile,
        region,
        secret_id,
        HashSet::from([subkey.to_string()]),
    )
    .await?
    .into_values()
    .next()
    .unwrap())
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

// Internal.
// ----------------------------------------------------------------------------

fn parse_secret_subkeys<T: DeserializeOwned>(
    raw: &str,
    secret_id: &str,
    region: &str,
    subkeys: &HashSet<String>,
) -> Result<HashMap<String, T>, CliError> {
    let secrets_map =
        serde_json::from_str::<HashMap<String, serde_json::Value>>(raw).map_err(|e| {
            FailedToFetchAwsSecret::with_debug(secret_id, region, "could not parse JSON", &e)
        })?;

    let result = secrets_map
        .into_iter()
        .filter(|(key, _)| subkeys.contains(key))
        .collect::<HashMap<_, _>>();

    if result.len() != subkeys.len() {
        let missing_keys = subkeys
            .difference(&result.keys().cloned().collect())
            .map(|k| k.to_string())
            .collect::<HashSet<_>>();
        return Err(AwsSecretSubkeysNotFound::new(
            secret_id,
            region,
            &missing_keys,
        ));
    }

    result
        .into_iter()
        .map(|(subkey, value)| {
            let value = serde_json::from_value(value).map_err(|e| {
                FailedToDeserializeAwsSecretSubkey::with_debug(secret_id, region, &subkey, &e)
            })?;
            Ok((subkey, value))
        })
        .collect()
}

// Tests.
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn selected_subkeys_can_be_returned_as_json_values() {
        let selected = parse_secret_subkeys::<serde_json::Value>(
            r#"{"plain":"value","structured":{"active":"new"}}"#,
            "secret",
            "region",
            &HashSet::from(["plain".to_owned(), "structured".to_owned()]),
        )
        .unwrap();

        assert_eq!(selected.get("plain"), Some(&json!("value")));
        assert_eq!(selected.get("structured"), Some(&json!({"active": "new"})));
    }

    #[test]
    fn selected_subkeys_deserialize_into_the_requested_type() {
        assert_eq!(
            parse_secret_subkeys::<String>(
                r#"{"key":"value"}"#,
                "secret",
                "region",
                &HashSet::from(["key".to_owned()]),
            )
            .unwrap(),
            HashMap::from([("key".to_owned(), "value".to_owned())])
        );
    }
}
