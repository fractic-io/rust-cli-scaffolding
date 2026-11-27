use std::time::Duration;

use aws_config::{
    profile::ProfileFileCredentialsProvider, timeout::TimeoutConfig, BehaviorVersion, Region,
    SdkConfig,
};

/// The SDK configuration for the named profile of `~/.aws/config` in `region`,
/// ready for any `aws_sdk_*::Client::new`.
pub async fn aws_config_from_profile(
    profile: impl Into<String>,
    region: impl Into<String>,
) -> SdkConfig {
    // The SDK's default timeouts are too short for large operations such as S3
    // transfers. Its 5 s connect timeout in particular trips on slow
    // connections or when many requests run at once.
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
