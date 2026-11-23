use std::time::Duration;

use aws_config::{
    profile::ProfileFileCredentialsProvider, timeout::TimeoutConfig, BehaviorVersion, Region,
    SdkConfig,
};

/// The SDK configuration for `profile` (a profile in `~/.aws/config`, such as
/// the one `require_aws_profile` returns) in `region`, ready for any
/// `aws_sdk_*::Client::new`.
pub async fn aws_config_from_profile(
    profile: impl Into<String>,
    region: impl Into<String>,
) -> SdkConfig {
    // The SDK's default timeouts are too short for large operations (S3
    // transfers, stack deployments); its 5s connect timeout in particular
    // trips on slow connections or with many concurrent requests.
    let timeout_config = TimeoutConfig::builder()
        .connect_timeout(Duration::from_secs(30))
        .operation_attempt_timeout(Duration::from_secs(300))
        .operation_timeout(Duration::from_secs(3600))
        .build();

    aws_config::defaults(BehaviorVersion::latest())
        .region(Region::new(region.into()))
        .timeout_config(timeout_config)
        .credentials_provider(
            ProfileFileCredentialsProvider::builder()
                .profile_name(profile)
                .build(),
        )
        .load()
        .await
}
