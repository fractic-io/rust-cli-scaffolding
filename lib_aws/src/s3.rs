use std::path::Path;

use aws_sdk_cloudformation::error::SdkError;
use aws_sdk_s3::{
    operation::head_bucket::HeadBucketError, types::CreateBucketConfiguration, Client,
};
use lib_core::{define_cli_error, CliError, IOError};

use crate::shared_config::config_from_profile;

define_cli_error!(S3Error, "Error running S3 command.");
define_cli_error!(S3InvalidUpload, "Invalid S3 upload request: {details}.", { details: &str });

pub async fn bucket_exists(profile: &str, region: &str, bucket: &str) -> Result<bool, CliError> {
    let client = Client::new(&config_from_profile(profile, region).await);
    let response = client.head_bucket().bucket(bucket).send().await;
    match response {
        Ok(_) => Ok(true),
        Err(SdkError::<HeadBucketError>::ServiceError(se)) if se.err().is_not_found() => Ok(false),
        Err(e) => Err(S3Error::with_debug(&e)),
    }
}

pub async fn create_bucket_if_not_exists(
    profile: &str,
    region: &str,
    bucket: &str,
) -> Result<(), CliError> {
    if !bucket_exists(profile, region, bucket).await? {
        let client = Client::new(&config_from_profile(profile, region).await);
        client
            .create_bucket()
            .bucket(bucket)
            .create_bucket_configuration(
                CreateBucketConfiguration::builder()
                    .location_constraint(region.into())
                    .build(),
            )
            .send()
            .await
            .map_err(|e| S3Error::with_debug(&e))?;
    }
    Ok(())
}

pub async fn upload_file_to_s3<P>(
    profile: &str,
    region: &str,
    bucket: &str,
    key: &str,
    file_path: P,
) -> Result<(), CliError>
where
    P: AsRef<Path>,
{
    if !file_path.as_ref().exists() {
        return Err(S3InvalidUpload::new("file does not exist"));
    }
    if !file_path.as_ref().is_file() {
        return Err(S3InvalidUpload::new("path is not a file"));
    }
    let client = Client::new(&config_from_profile(profile, region).await);
    let body = aws_sdk_s3::primitives::ByteStream::from_path(file_path)
        .await
        .map_err(|e| IOError::with_debug(&e))?;
    client
        .put_object()
        .bucket(bucket)
        .key(key)
        .body(body)
        .send()
        .await
        .map_err(|e| S3Error::with_debug(&e))?;
    Ok(())
}

pub async fn upload_dir_to_s3<P>(
    profile: &str,
    region: &str,
    bucket: &str,
    key_prefix: &str,
    dir_path: P,
) -> Result<(), CliError>
where
    P: AsRef<Path>,
{
    if !dir_path.as_ref().exists() {
        return Err(S3InvalidUpload::new("directory does not exist"));
    }
    if !dir_path.as_ref().is_dir() {
        return Err(S3InvalidUpload::new("path is not a directory"));
    }
    let client = Client::new(&config_from_profile(profile, region).await);
    for entry in walkdir::WalkDir::new(&dir_path) {
        let entry = entry.map_err(|e| IOError::with_debug(&e))?;
        if entry.file_type().is_file() {
            let key = format!(
                "{}/{}",
                key_prefix,
                entry
                    .path()
                    .strip_prefix(dir_path.as_ref())
                    .unwrap()
                    .to_string_lossy()
            );
            let body = aws_sdk_s3::primitives::ByteStream::from_path(entry.path())
                .await
                .map_err(|e| IOError::with_debug(&e))?;
            client
                .put_object()
                .bucket(bucket)
                .key(&key)
                .body(body)
                .send()
                .await
                .map_err(|e| S3Error::with_debug(&e))?;
        }
    }
    Ok(())
}
