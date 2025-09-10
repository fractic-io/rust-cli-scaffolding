use aws_config::{profile::ProfileFileCredentialsProvider, BehaviorVersion, Region, SdkConfig};

pub(crate) async fn config_from_profile(
    profile: impl Into<String>,
    region: impl Into<String>,
) -> SdkConfig {
    aws_config::defaults(BehaviorVersion::v2025_08_07())
        .region(Region::new(region.into()))
        .credentials_provider(
            ProfileFileCredentialsProvider::builder()
                .profile_name(profile)
                .build(),
        )
        .load()
        .await
}
