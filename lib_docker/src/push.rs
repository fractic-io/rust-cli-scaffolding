use std::io::{self, Write};

use bollard::{auth::DockerCredentials, image::PushImageOptions, Docker};
use futures_util::TryStreamExt as _;
use lib_aws::EcrCredentials;
use lib_core::{define_cli_error, CliError, Printer};

use crate::DockerConnectionError;

define_cli_error!(DockerPushError, "Failed to push Docker image.");

pub async fn push_docker_image_to_ecr(
    pr: &Printer,
    ecr_repo: &str,
    tag: &str,
    credentials: EcrCredentials,
) -> Result<(), CliError> {
    let push_opts = PushImageOptions {
        tag,
        ..Default::default()
    };

    let creds = DockerCredentials {
        username: Some(credentials.username),
        password: Some(credentials.password),
        serveraddress: Some(credentials.proxy_endpoint),
        ..Default::default()
    };

    pr.info(&format!(
        "Pushing Docker image '{}:{}' to ECR...",
        ecr_repo, tag
    ));

    let docker =
        Docker::connect_with_local_defaults().map_err(|e| DockerConnectionError::with_debug(&e))?;
    let mut push_stream = docker.push_image(ecr_repo, Some(push_opts), Some(creds));

    let stdout = io::stdout();
    let mut handle = stdout.lock();
    while let Some(chunk) = push_stream
        .try_next()
        .await
        .map_err(|e| DockerPushError::with_debug(&e))?
    {
        if let Some(status) = chunk.status {
            write!(handle, "\r\x1b[2K").unwrap();
            write!(
                handle,
                "\r{}; {}",
                status,
                chunk.progress.unwrap_or_default()
            )
            .unwrap();
            handle.flush().unwrap();
        }
    }
    write!(handle, "\n").unwrap();
    handle.flush().unwrap();

    pr.info("Image pushed successfully.");

    Ok(())
}
