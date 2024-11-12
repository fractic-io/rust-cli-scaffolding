use aws_config::{profile::ProfileFileCredentialsProvider, BehaviorVersion, Region};
use aws_sdk_sts::Client;
use textwrap::fill;

const TEST_REGION: &str = "us-west-1";

pub async fn require_identity(
    sso_session: &str,
    account_id: &str,
    cli_role: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let profile = format!("{}-{}", cli_role, account_id);
    let shared_config = aws_config::defaults(BehaviorVersion::v2024_03_28())
        .region(Region::new(TEST_REGION))
        .credentials_provider(
            ProfileFileCredentialsProvider::builder()
                .profile_name(&profile)
                .build(),
        )
        .load()
        .await;
    let client = Client::new(&shared_config);
    client.get_caller_identity().send().await.map_err(|_| {
        fill(&format!(
            "This script requires AWS CLI profile {profile} ({cli_role} role for the account ID {account_id}). If the profile is not yet set up, please run:\n\n$ aws configure sso\n\nOr, if the profile is already set up but the token has expired simply log in again (required daily):\n\n$ aws sso login --sso-session {sso_session}"
        ), 80)
    })?;
    Ok(())
}
