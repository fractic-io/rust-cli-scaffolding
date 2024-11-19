use aws_sdk_sts::Client;
use lib_core::{define_cli_error, CliError};

use crate::shared_config::config_from_profile;

const TEST_REGION: &str = "us-west-1";

define_cli_error!(
    AwsProfileRequired,
    "This script requires AWS CLI profile {profile} ({cli_role} role for the account ID {account_id}). If the profile is not yet set up, please run:\n\n$ aws configure sso\n\nOr, if the profile is already set up but the token has expired simply log in again (required daily):\n\n$ aws sso login --sso-session {sso_session}",
    { profile: &str, cli_role: &str, account_id: &str, sso_session: &str }
);

pub async fn require_aws_profile(
    sso_session: &str,
    account_id: &str,
    cli_role: &str,
) -> Result<String, CliError> {
    let profile = format!("{}-{}", cli_role, account_id);
    let client = Client::new(&config_from_profile(&profile, TEST_REGION).await);
    client.get_caller_identity().send().await.map_err(|_| {
        AwsProfileRequired::new(&profile, cli_role, &account_id.to_string(), sso_session)
    })?;
    Ok(profile)
}
