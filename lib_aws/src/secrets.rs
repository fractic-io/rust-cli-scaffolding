use std::collections::HashMap;

use aws_config::{profile::ProfileFileCredentialsProvider, BehaviorVersion, Region};
use aws_sdk_secretsmanager::Client;

pub async fn get_secret(
    account_id: &str,
    cli_role: &str,
    region: &str,
    secret_id: &str,
    key: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let profile_name = format!("{}-{}", cli_role, account_id);
    let shared_config = aws_config::defaults(BehaviorVersion::v2024_03_28())
        .region(Region::new(region.to_string()))
        .credentials_provider(
            ProfileFileCredentialsProvider::builder()
                .profile_name(profile_name)
                .build(),
        )
        .load()
        .await;
    let client = Client::new(&shared_config);

    // Fetch secrets JSON.
    let secrets_output = client
        .get_secret_value()
        .secret_id(secret_id)
        .send()
        .await?;
    let secrets_string = secrets_output.secret_string().ok_or(format!(
        "Could not parse secret value.\nSecretsId: {}; Region: {};",
        secret_id, region
    ))?;
    let secrets_json =
        serde_json::from_str::<HashMap<String, String>>(secrets_string).map_err(|e| {
            format!(
                "SecretsId: {}; Region: {}; Error: {};",
                secret_id,
                region,
                e.to_string()
            )
        })?;

    // Fetch required keys from JSON.
    Ok(secrets_json
        .get(key)
        .cloned()
        .ok_or_else(|| format!("Secret '{secret_id}' did not contain key '{key}'."))?)
}
