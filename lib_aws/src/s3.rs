use std::path::Path;

use aws_sdk_cloudformation::error::SdkError;
use aws_sdk_s3::{operation::head_bucket::HeadBucketError, types::CreateBucketConfiguration, Client};
use chrono::{DateTime, Utc};
use lib_core::{define_cli_error, CliError, IOError, Printer};
use sha2::{Digest as _, Sha256};

use crate::shared_config::config_from_profile;

define_cli_error!(S3Error, "Error running S3 command.");
define_cli_error!(S3InvalidUpload, "Invalid S3 upload request: {details}.", { details: &str });

#[derive(Debug, Clone)]
pub struct S3ObjectMetadata {
    pub key: String,
    pub size: i64,
    pub etag: Option<String>,
    pub last_modified: Option<DateTime<Utc>>,
}

pub async fn bucket_exists(profile: &str, region: &str, bucket: &str) -> Result<bool, CliError> {
    let client = Client::new(&config_from_profile(profile, region).await);
    let response = client.head_bucket().bucket(bucket).send().await;
    match response {
        Ok(_) => Ok(true),
        Err(SdkError::<HeadBucketError>::ServiceError(se)) if se.err().is_not_found() => Ok(false),
        Err(e) => Err(S3Error::with_debug(&e)),
    }
}

/// Returns true if new bucket was created.
pub async fn create_bucket_if_not_exists(
    pr: &Printer,
    profile: &str,
    region: &str,
    bucket: &str,
) -> Result<bool, CliError> {
    if !bucket_exists(profile, region, bucket).await? {
        pr.info(&format!("Creating S3 bucket '{}'...", bucket));
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
        pr.info("Bucket created.");
        Ok(true)
    } else {
        pr.info(&format!("Bucket '{}' already exists.", bucket));
        Ok(false)
    }
}

pub async fn upload_file_to_s3<P>(
    pr: &Printer,
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
    pr.info(&format!("Uploaded file to 's3://{}/{}'.", bucket, key));
    Ok(())
}

pub async fn list_objects_with_prefix(
    profile: &str,
    region: &str,
    bucket: &str,
    prefix: &str,
) -> Result<Vec<S3ObjectMetadata>, CliError> {
    let client = Client::new(&config_from_profile(profile, region).await);
    let mut objects = Vec::new();
    let mut continuation_token: Option<String> = None;

    loop {
        let response = client
            .list_objects_v2()
            .bucket(bucket)
            .prefix(prefix)
            .set_continuation_token(continuation_token.clone())
            .send()
            .await
            .map_err(|e| S3Error::with_debug(&e))?;

        for object in response.contents.unwrap_or_default() {
            if let Some(key) = object.key {
                let last_modified = object
                    .last_modified
                    .and_then(|dt| chrono::DateTime::from_timestamp(dt.secs(), 0))
                    .map(|dt| dt.with_timezone(&Utc));
                objects.push(S3ObjectMetadata {
                    key,
                    size: object.size.unwrap_or_default(),
                    etag: object.e_tag.map(|x| x.trim_matches('"').to_string()),
                    last_modified,
                });
            }
        }

        if response.is_truncated.unwrap_or(false) {
            continuation_token = response.next_continuation_token;
        } else {
            break;
        }
    }

    Ok(objects)
}

pub async fn download_object_to_path<P>(
    profile: &str,
    region: &str,
    bucket: &str,
    key: &str,
    local_path: P,
) -> Result<(), CliError>
where
    P: AsRef<Path>,
{
    let client = Client::new(&config_from_profile(profile, region).await);
    let response = client
        .get_object()
        .bucket(bucket)
        .key(key)
        .send()
        .await
        .map_err(|e| S3Error::with_debug(&e))?;
    let bytes = response
        .body
        .collect()
        .await
        .map_err(|e| IOError::with_debug(&e))?
        .into_bytes();

    if let Some(parent) = local_path.as_ref().parent() {
        std::fs::create_dir_all(parent).map_err(|e| IOError::with_debug(&e))?;
    }
    std::fs::write(local_path, bytes).map_err(|e| IOError::with_debug(&e))?;
    Ok(())
}

pub async fn delete_object(profile: &str, region: &str, bucket: &str, key: &str) -> Result<(), CliError> {
    let client = Client::new(&config_from_profile(profile, region).await);
    client
        .delete_object()
        .bucket(bucket)
        .key(key)
        .send()
        .await
        .map_err(|e| S3Error::with_debug(&e))?;
    Ok(())
}

pub async fn delete_objects(
    profile: &str,
    region: &str,
    bucket: &str,
    keys: &[String],
) -> Result<(), CliError> {
    for key in keys {
        delete_object(profile, region, bucket, key).await?;
    }
    Ok(())
}

pub async fn create_folder_placeholder(
    pr: &Printer,
    profile: &str,
    region: &str,
    bucket: &str,
    key: &str,
) -> Result<(), CliError> {
    let client = Client::new(&config_from_profile(profile, region).await);
    client
        .put_object()
        .bucket(bucket)
        .key(key)
        .body(aws_sdk_s3::primitives::ByteStream::from_static(&[]))
        .send()
        .await
        .map_err(|e| S3Error::with_debug(&e))?;
    pr.info(&format!(
        "Created folder placeholder at 's3://{}/{}'.",
        bucket, key
    ));
    Ok(())
}

/// Returns the number of files uploaded.
pub async fn upload_dir_to_s3<P>(
    pr: &Printer,
    profile: &str,
    region: &str,
    bucket: &str,
    key_prefix: &str,
    dir_path: P,
) -> Result<usize, CliError>
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
    let mut count = 0;
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
            count += 1;
        }
    }
    pr.info(&format!(
        "Uploaded {} file(s) to 's3://{}/{}/'.",
        count, bucket, key_prefix
    ));
    Ok(count)
}

/// Deterministically derives a unique bucket name.
pub fn derive_unique_bucket_name(
    sso_session: &str,
    account_id: &str,
    region: &str,
    prefix: &str,
    project_id: &str,
) -> String {
    // This should be globally unique, so we must take care to incorporate the
    // company name and account ID.
    let deterministic_hash = hex::encode(Sha256::digest(
        format!("{sso_session}.{account_id}.{region}.{project_id}").as_bytes(),
    ));
    format!("{prefix}-{}", &deterministic_hash[..32])
}
