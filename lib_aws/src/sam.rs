use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use lib_core::{ln_s, rm_rf, CliError, ExecuteOptions, Executor, IOMode, Printer};

pub async fn sam_build(pr: &Printer, ex: &Executor, project_dir: &Path) -> Result<(), CliError> {
    pr.info("Building with SAM...");
    if let Ok(ref workdir) = std::env::var("CARGO_LAMBDA_BUILD_DIR") {
        // Allow overriding the build directory with an environment variable,
        // which sets both the SAM output directory, as well as the Cargo target
        // directory.
        pr.info(&format!("Using custom build dir: '{}'.", workdir));
        let sam_build_dir = PathBuf::from(workdir).join("build");
        let cargo_target_dir = PathBuf::from(workdir).join("target");

        // It seems SAM isn't fully compatible with a custom CARGO_TARGET_DIR,
        // so we need to symlink the expected '/target' directory to our custom
        // location.
        let expected_target_dir = project_dir.join("code").join("target");
        rm_rf(&expected_target_dir)?;
        ln_s(&cargo_target_dir, &expected_target_dir)?;

        ex.execute_with_options(
            "sam",
            &[
                "build",
                "--build-dir",
                sam_build_dir.to_string_lossy().as_ref(),
            ],
            IOMode::Attach,
            ExecuteOptions {
                dir: Some(project_dir),
                env: Some(vec![(
                    "CARGO_TARGET_DIR".to_string(),
                    cargo_target_dir.to_string_lossy().to_string(),
                )]),
                ..Default::default()
            },
        )?;
    } else {
        ex.execute_with_options(
            "sam",
            &["build"],
            IOMode::Attach,
            ExecuteOptions {
                dir: Some(project_dir),
                ..Default::default()
            },
        )?;
    }
    Ok(())
}

pub async fn sam_deploy(
    pr: &Printer,
    ex: &Executor,
    project_dir: &Path,
    profile: &str,
    region: &str,
    stack_name: &str,
    parameter_overrides: HashMap<String, String>,
) -> Result<(), CliError> {
    let param_strs = parameter_overrides
        .into_iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>();
    let args = vec![
        "deploy",
        "--profile",
        profile,
        "--region",
        region,
        "--stack-name",
        stack_name,
        "--no-fail-on-empty-changeset",
        "--parameter-overrides",
    ]
    .into_iter()
    .map(String::from)
    .chain(param_strs.into_iter())
    .collect::<Vec<_>>();

    pr.info("Deploying with SAM...");
    ex.execute_with_options(
        "sam",
        &args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        IOMode::Attach,
        ExecuteOptions {
            dir: Some(project_dir),
            ..Default::default()
        },
    )?;
    Ok(())
}
