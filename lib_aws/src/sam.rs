use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use lib_core::{CliError, ExecuteOptions, Executor, IOMode, Printer};

pub async fn sam_build(pr: &Printer, ex: &Executor, project_dir: &Path) -> Result<(), CliError> {
    pr.info("Building with SAM...");
    if let Ok(ref workdir) = std::env::var("CARGO_LAMBDA_BUILD_DIR") {
        pr.info(&format!("Using build dir: '{}'", workdir));
        let sam_build_dir = PathBuf::from(workdir).join("build");
        let cargo_target_dir = PathBuf::from(workdir).join("target");
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
