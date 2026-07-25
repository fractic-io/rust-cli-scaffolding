use std::path::Path;

use lib_core::{CliError, CriticalError, ExecuteOptions, Executor, IOMode, Printer};
use tempfile::tempdir;

use crate::lambda_build::{lambda_build_env, LAMBDA_AARCH64_TARGET};
use crate::s3_upload_dir;

pub async fn cargo_lambda_build_to_s3(
    pr: &Printer,
    ex: &Executor,
    crate_dir: &Path,
    profile: &str,
    region: &str,
    bucket: &str,
    key_prefix: &str,
) -> Result<(), CliError> {
    pr.info("Building binaries...");
    let target_dir =
        tempdir().map_err(|e| CriticalError::with_debug("failed to get temp dir", &e))?;
    // Pin AL2023 glibc explicitly (do not use --arm64 alone): on Linux hosts that
    // share the aarch64 triple, bare --arm64 can native-link against host glibc.
    ex.execute_with_options(
        "cargo",
        &[
            "lambda",
            "build",
            "--output-format",
            "zip",
            "--lambda-dir",
            target_dir.path().to_str().ok_or_else(|| {
                CriticalError::new("failed to convert path from tempdir() to string")
            })?,
            "--release",
            "--target",
            LAMBDA_AARCH64_TARGET,
        ],
        IOMode::Attach,
        ExecuteOptions {
            dir: Some(crate_dir),
            env: Some(lambda_build_env()),
            ..Default::default()
        },
    )
    .await?;
    pr.info("Uploading zip files to S3...");
    s3_upload_dir(pr, profile, region, bucket, key_prefix, target_dir.path()).await?;
    Ok(())
}
