use std::path::Path;

use lib_core::{CliError, CriticalError, Executor, IOMode, Printer};
use tempfile::tempdir;

use crate::upload_dir_to_s3;

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
    ex.execute(
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
            "--arm64",
        ],
        Some(crate_dir),
        IOMode::Attach,
    )?;
    pr.info("Uploading zip files to S3...");
    upload_dir_to_s3(pr, profile, region, bucket, key_prefix, target_dir.path()).await?;
    Ok(())
}
