use aws_sdk_iam::{
    types::{AccessKeyMetadata, StatusType},
    Client,
};
use lib_core::{define_cli_error, CliError, Printer};

use crate::shared_config::config_from_profile;

define_cli_error!(IamError, "Error running AWS IAM command.");

pub struct IamAccessKeyCredentials {
    pub access_key_id: String,
    pub secret_access_key: String,
}

fn select_access_key_to_delete(access_keys: &[AccessKeyMetadata]) -> Option<String> {
    access_keys
        .iter()
        .find(|key| key.status() == Some(&StatusType::Inactive))
        .or_else(|| access_keys.first())
        .and_then(|key| key.access_key_id().map(|v| v.to_string()))
}

/// Creates a fresh access key for an IAM user.
///
/// IAM supports at most 2 active keys per user. If that limit is reached,
/// this deletes one existing key (preferring inactive keys) before creating
/// a new one.
pub async fn create_fresh_access_key_for_user(
    printer: &Printer,
    profile: &str,
    region: &str,
    username: &str,
) -> Result<IamAccessKeyCredentials, CliError> {
    let client = Client::new(&config_from_profile(profile, region).await);

    let existing_keys = client
        .list_access_keys()
        .user_name(username)
        .send()
        .await
        .map_err(|e| IamError::with_debug(&e))?
        .access_key_metadata;

    if existing_keys.len() >= 2 {
        if let Some(key_to_delete) = select_access_key_to_delete(&existing_keys) {
            printer.info(&format!(
                "User '{}' already has 2 access keys. Deleting key '{}' to rotate credentials...",
                username, key_to_delete
            ));
            client
                .delete_access_key()
                .user_name(username)
                .access_key_id(key_to_delete)
                .send()
                .await
                .map_err(|e| IamError::with_debug(&e))?;
        }
    }

    let key = client
        .create_access_key()
        .user_name(username)
        .send()
        .await
        .map_err(|e| IamError::with_debug(&e))?
        .access_key
        .ok_or_else(|| IamError::new())?;

    Ok(IamAccessKeyCredentials {
        access_key_id: key.access_key_id().to_string(),
        secret_access_key: key.secret_access_key().to_string(),
    })
}
