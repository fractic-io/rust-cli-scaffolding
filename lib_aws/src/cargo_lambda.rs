use std::path::Path;

use lib_core::{CliError, Executor, IOMode, Printer};

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
    ex.execute(
        "cargo",
        &[
            "lambda",
            "build",
            "--output-format",
            "zip",
            "--release",
            "--arm64",
        ],
        Some(crate_dir),
        IOMode::Attach,
    )?;
    pr.info("Uploading zip files to S3...");
    upload_dir_to_s3(
        pr,
        profile,
        region,
        bucket,
        key_prefix,
        crate_dir.join("target").join("lambda"),
    )
    .await?;
    Ok(())
}
