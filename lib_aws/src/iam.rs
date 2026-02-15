use aws_sdk_iam::{
    types::{AccessKeyMetadata, StatusType},
    Client,
};
use lib_core::{define_cli_error, CliError, Printer};

use crate::shared_config::config_from_profile;

define_cli_error!(IamError, "Error running AWS IAM command.");
define_cli_error!(
    IamAccessKeyLimitReached,
    "User '{username}' already has {key_count} access keys and key rotation mode is None.",
    { username: &str, key_count: usize }
);
define_cli_error!(
    IamAccessKeyNullAfterCreate,
    "Access key returned from AWS IAM was null."
);

pub struct IamAccessKeyCredentials {
    pub access_key_id: String,
    pub secret_access_key: String,
}

#[derive(Clone, Copy, Debug)]
pub enum KeyRotation {
    /// If max active keys is exceeded, delete the oldest key.
    DeleteOldest,
    /// If max active keys is exceeded, delete the newest key.
    DeleteNewest,
    /// Always delete all existing keys when generating a new access key.
    DeleteAll,
    /// Do not touch existing active keys. This will cause an error if the user
    /// already has the max number of active keys.
    None,
}

/// Creates a fresh access key for an IAM user.
///
/// IAM supports at most 2 active keys per user. Set `rotation` to specify how
/// rotation of existing keys should be handled.
pub async fn create_access_key_for_user(
    printer: &Printer,
    profile: &str,
    region: &str,
    username: &str,
    rotation: KeyRotation,
) -> Result<IamAccessKeyCredentials, CliError> {
    let client = Client::new(&config_from_profile(profile, region).await);
    let mut existing_keys = list_access_keys(&client, username).await?;

    // Clean up any inactive keys.
    let inactive_key_ids = existing_keys
        .iter()
        .filter(|key| key.status() == Some(&StatusType::Inactive))
        .filter_map(key_id)
        .collect::<Vec<_>>();
    for access_key_id in inactive_key_ids {
        delete_access_key(printer, &client, username, &access_key_id).await?;
    }

    existing_keys = list_access_keys(&client, username).await?;

    match rotation {
        KeyRotation::DeleteAll => {
            for access_key_id in existing_keys.iter().filter_map(key_id).collect::<Vec<_>>() {
                delete_access_key(printer, &client, username, &access_key_id).await?;
            }
        }
        KeyRotation::DeleteOldest => {
            if existing_keys.len() >= 2 {
                if let Some(access_key_id) = pick_oldest_key(&existing_keys) {
                    delete_access_key(printer, &client, username, &access_key_id).await?;
                }
            }
        }
        KeyRotation::DeleteNewest => {
            if existing_keys.len() >= 2 {
                if let Some(access_key_id) = pick_newest_key(&existing_keys) {
                    delete_access_key(printer, &client, username, &access_key_id).await?;
                }
            }
        }
        KeyRotation::None => {
            if existing_keys.len() >= 2 {
                return Err(IamAccessKeyLimitReached::new(username, existing_keys.len()));
            }
        }
    }

    let key = client
        .create_access_key()
        .user_name(username)
        .send()
        .await
        .map_err(|e| IamError::with_debug(&e))?
        .access_key
        .ok_or_else(|| IamAccessKeyNullAfterCreate::new())?;

    Ok(IamAccessKeyCredentials {
        access_key_id: key.access_key_id,
        secret_access_key: key.secret_access_key,
    })
}

// Helpers.
// ----------------------------------------------------------------------------

fn key_id(key: &AccessKeyMetadata) -> Option<String> {
    key.access_key_id().map(|v| v.to_string())
}

fn created_at_epoch_seconds(key: &AccessKeyMetadata) -> i64 {
    key.create_date().map(|v| v.secs()).unwrap_or(i64::MIN)
}

fn pick_oldest_key(access_keys: &[AccessKeyMetadata]) -> Option<String> {
    access_keys
        .iter()
        .min_by_key(|key| created_at_epoch_seconds(key))
        .and_then(key_id)
}

fn pick_newest_key(access_keys: &[AccessKeyMetadata]) -> Option<String> {
    access_keys
        .iter()
        .max_by_key(|key| created_at_epoch_seconds(key))
        .and_then(key_id)
}

async fn delete_access_key(
    printer: &Printer,
    client: &Client,
    username: &str,
    access_key_id: &str,
) -> Result<(), CliError> {
    printer.info(&format!(
        "Deleting access key '{}' for user '{}'...",
        access_key_id, username
    ));
    client
        .delete_access_key()
        .user_name(username)
        .access_key_id(access_key_id)
        .send()
        .await
        .map_err(|e| IamError::with_debug(&e))?;
    Ok(())
}

async fn list_access_keys(
    client: &Client,
    username: &str,
) -> Result<Vec<AccessKeyMetadata>, CliError> {
    Ok(client
        .list_access_keys()
        .user_name(username)
        .send()
        .await
        .map_err(|e| IamError::with_debug(&e))?
        .access_key_metadata)
}
