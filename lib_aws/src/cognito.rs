use aws_sdk_cognitoidentityprovider::{
    error::SdkError,
    operation::admin_get_user::AdminGetUserError,
    types::{AttributeType, MessageActionType},
    Client,
};
use lib_core::{define_cli_error, CliError, Printer};

use crate::shared_config::config_from_profile;

define_cli_error!(CognitoError, "Error running AWS Cognito command.");

pub async fn user_exists(
    profile: &str,
    region: &str,
    user_pool_id: &str,
    username: &str,
) -> Result<bool, CliError> {
    let client = Client::new(&config_from_profile(profile, region).await);
    let response = client
        .admin_get_user()
        .user_pool_id(user_pool_id)
        .username(username)
        .send()
        .await;
    match response {
        Ok(_) => Ok(true),
        Err(SdkError::<AdminGetUserError>::ServiceError(se))
            if se.err().is_user_not_found_exception() =>
        {
            Ok(false)
        }
        Err(e) => Err(CognitoError::with_debug(&e)),
    }
}

/// Returns true if new user was created.
pub async fn create_user_if_not_exists(
    printer: &Printer,
    profile: &str,
    region: &str,
    user_pool_id: &str,
    username: &str,
    email: &str,
    password: &str,
) -> Result<bool, CliError> {
    if user_exists(profile, region, user_pool_id, username).await? {
        printer.info(&format!("User '{}' already exists.", username));
        Ok(false)
    } else {
        printer.info(&format!("Creating user '{}'...", username));
        let client = Client::new(&config_from_profile(profile, region).await);
        client
            .admin_create_user()
            .user_pool_id(user_pool_id)
            .username(username)
            .user_attributes(
                AttributeType::builder()
                    .name("email")
                    .value(email)
                    .build()
                    .map_err(|e| CognitoError::with_debug(&e))?,
            )
            .user_attributes(
                AttributeType::builder()
                    .name("email_verified")
                    .value("true")
                    .build()
                    .map_err(|e| CognitoError::with_debug(&e))?,
            )
            .temporary_password(password)
            .message_action(MessageActionType::Suppress)
            .send()
            .await
            .map_err(|e| CognitoError::with_debug(&e))?;
        client
            .admin_set_user_password()
            .user_pool_id(user_pool_id)
            .username(username)
            .password(password)
            .permanent(true)
            .send()
            .await
            .map_err(|e| CognitoError::with_debug(&e))?;
        printer.info("User created.");
        Ok(true)
    }
}

pub async fn add_user_to_group(
    pr: &Printer,
    profile: &str,
    region: &str,
    user_pool_id: &str,
    username: &str,
    group: &str,
) -> Result<(), CliError> {
    pr.info(&format!(
        "Adding user '{}' to group '{}'...",
        username, group
    ));
    let client = Client::new(&config_from_profile(profile, region).await);
    client
        .admin_add_user_to_group()
        .user_pool_id(user_pool_id)
        .username(username)
        .group_name(group)
        .send()
        .await
        .map_err(|e| CognitoError::with_debug(&e))?;
    pr.info("Added.");
    Ok(())
}
