use std::time::Duration;

use aws_config::{
    profile::ProfileFileCredentialsProvider, timeout::TimeoutConfig, BehaviorVersion, Region,
    SdkConfig,
};

pub(crate) async fn config_from_profile(
    profile: impl Into<String>,
    region: impl Into<String>,
) -> SdkConfig {
    // Important to raise the default timeouts to support large operations
    // (e.g., large S3 file transfers). In particular, the default 5s connect
    // timeout can easily trigger on slower connections or with multiple
    // concurrent operations.
    let timeout_config = TimeoutConfig::builder()
        .connect_timeout(Duration::from_secs(30))
        .operation_attempt_timeout(Duration::from_secs(300))
        .operation_timeout(Duration::from_secs(3600))
        .build();

    aws_config::defaults(BehaviorVersion::v2026_01_12())
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
