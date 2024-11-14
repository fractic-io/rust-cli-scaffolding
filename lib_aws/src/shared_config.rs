use aws_config::{profile::ProfileFileCredentialsProvider, BehaviorVersion, Region, SdkConfig};

pub(crate) async fn config_from_profile(
    region: impl Into<String>,
    profile: impl Into<String>,
) -> SdkConfig {
    aws_config::defaults(BehaviorVersion::v2024_03_28())
        .region(Region::new(region.into()))
        .credentials_provider(
            ProfileFileCredentialsProvider::builder()
                .profile_name(profile)
                .build(),
        )
        .load()
        .await
}
