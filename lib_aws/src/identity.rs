use aws_sdk_sts::{error::ProvideErrorMetadata, Client};
use aws_smithy_runtime_api::client::result::SdkError;
use lib_core::{define_cli_error, CliError};

use crate::shared_config::config_from_profile;

const TEST_REGION: &str = "us-west-1";

define_cli_error!(
    AwsProfileRequired,
    "This script requires AWS CLI profile {profile} ({cli_role} role for the account ID {account_id}). If the profile is not yet set up, please run:\n\n$ aws configure sso\n\nOr, if the profile is already set up but the token has expired simply log in again (required daily):\n\n$ aws sso login --sso-session {sso_session}",
    { profile: &str, cli_role: &str, account_id: &str, sso_session: &str }
);

define_cli_error!(
    AwsProfileCheckFailed,
    "Could not verify AWS CLI profile {profile}. This looks like an AWS/network error, not necessarily an expired SSO login.",
    { profile: &str }
);

pub async fn require_aws_profile(
    sso_session: &str,
    account_id: &str,
    cli_role: &str,
) -> Result<String, CliError> {
    let profile = format!("{}-{}", cli_role, account_id);
    let client = Client::new(&config_from_profile(&profile, TEST_REGION).await);
    client.get_caller_identity().send().await.map_err(|e| {
        if is_aws_profile_required_error(&e) {
            AwsProfileRequired::with_debug(&profile, cli_role, account_id, sso_session, &e)
        } else {
            AwsProfileCheckFailed::with_debug(&profile, &e)
        }
    })?;
    Ok(profile)
}

fn is_aws_profile_required_error<E, R>(error: &SdkError<E, R>) -> bool
where
    E: ProvideErrorMetadata,
{
    match error {
        SdkError::ConstructionFailure(_) => false,
        SdkError::ServiceError(e) => e
            .err()
            .code()
            .map(is_aws_auth_or_credentials_error_code)
            .unwrap_or(false),
        SdkError::DispatchFailure(_) | SdkError::TimeoutError(_) | SdkError::ResponseError(_) => {
            false
        }
        _ => false,
    }
}

fn is_aws_auth_or_credentials_error_code(code: &str) -> bool {
    matches!(
        code,
        "AccessDenied"
            | "AccessDeniedException"
            | "ExpiredToken"
            | "ExpiredTokenException"
            | "InvalidClientTokenId"
            | "InvalidToken"
            | "RequestExpired"
            | "TokenRefreshRequired"
            | "UnauthorizedException"
            | "UnrecognizedClientException"
    )
}
