use std::{io::Write as _, path::Path};

use bollard::{
    image::{BuildImageOptions, TagImageOptions},
    Docker,
};
use futures_util::TryStreamExt as _;
use lib_core::{
    build_tar_bundle, define_cli_error, CliError, CriticalError, Executor, IOMode, InvalidUTF8,
    Printer,
};
use tempfile::tempdir;
use tokio::{fs::File, io::AsyncReadExt as _};

use crate::DockerConnectionError;

define_cli_error!(
    DockerfileNotFound,
    "Dockerfile not found at path: {path:?}.",
    { path: &Path }
);
define_cli_error!(DockerBuildError, "Failed to build Docker image.");
define_cli_error!(DockerTagError, "Failed to tag Docker image.");

pub async fn build_docker_image_with_bullard<P: AsRef<Path>>(
    pr: &Printer,
    dockerfile_path: P,
    image_name: &str,
) -> Result<(), CliError> {
    if !dockerfile_path.as_ref().exists() {
        return Err(DockerfileNotFound::new(dockerfile_path.as_ref()));
    }

    let context_dir = dockerfile_path
        .as_ref()
        .parent()
        .ok_or_else(|| CriticalError::new("Dockerfile does not have parent directory"))?;
    let dockerfile_name = dockerfile_path
        .as_ref()
        .file_name()
        .ok_or_else(|| CriticalError::new("Dockerfile does not have a valid filename"))?
        .to_str()
        .ok_or_else(|| InvalidUTF8::new())?;

    pr.info("Building tar bundle...");
    let tmp_dir = tempdir().map_err(|e| {
        CriticalError::with_debug(
            "failed to create temporary directory for building tar bundle",
            &e,
        )
    })?;
    let bundle_path = build_tar_bundle(context_dir, &tmp_dir)?;
    let mut bundle = File::open(&bundle_path).await.map_err(|e| {
        CriticalError::with_debug(
            "failed to open tar bundle, even though it was only just created",
            &e,
        )
    })?;
    let mut bundle_bytes = Vec::new();
    bundle
        .read_to_end(&mut bundle_bytes)
        .await
        .map_err(|e| CriticalError::with_debug("failed to read tar bundle", &e))?;

    pr.info(&format!(
        "Building Docker image '{}' from '{}'...",
        image_name,
        dockerfile_path.as_ref().display()
    ));
    let docker =
        Docker::connect_with_local_defaults().map_err(|e| DockerConnectionError::with_debug(&e))?;
    let build_opts = BuildImageOptions {
        dockerfile: dockerfile_name,
        t: image_name,
        rm: true,
        ..Default::default()
    };
    let mut build_stream = docker.build_image(build_opts, None, Some(bundle_bytes.into()));
    while let Some(chunk) = build_stream
        .try_next()
        .await
        .map_err(|e| DockerBuildError::with_debug(&e))?
    {
        if let Some(stream) = chunk.stream {
            print!("{}", stream); // Print build output.
            std::io::stdout().flush().unwrap();
        }
    }
    pr.info("Image built successfully.");

    Ok(())
}

pub fn build_docker_image_with_command_line<P: AsRef<Path>>(
    pr: &Printer,
    ex: &Executor,
    build_dir: P,
    image_name: &str,
) -> Result<(), CliError> {
    let dockerfile_path = build_dir.as_ref().join("Dockerfile");
    if !dockerfile_path.exists() {
        return Err(DockerfileNotFound::new(&dockerfile_path));
    }
    pr.info(&format!(
        "Building Docker image '{}' from '{}'...",
        image_name,
        dockerfile_path.display()
    ));
    ex.execute(
        "docker",
        &["build", "-t", image_name, "."],
        Some(build_dir.as_ref()),
        IOMode::Attach,
    )
    .map_err(|e| DockerBuildError::with_debug(&e))?;
    pr.info("Image built successfully.");
    Ok(())
}

pub async fn tag_docker_image_for_ecr(
    pr: &Printer,
    image_name: &str,
    ecr_repo: &str,
    tag: &str,
) -> Result<(), CliError> {
    let tag_opts = TagImageOptions {
        repo: ecr_repo,
        tag,
    };
    pr.info(&format!(
        "Tagging image '{}' as '{}:{}'...",
        image_name, ecr_repo, tag
    ));
    let docker =
        Docker::connect_with_local_defaults().map_err(|e| DockerConnectionError::with_debug(&e))?;
    docker
        .tag_image(image_name, Some(tag_opts))
        .await
        .map_err(|e| DockerTagError::with_debug(&e))?;
    Ok(())
}
