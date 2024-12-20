use std::{collections::HashMap, path::Path};

use lib_core::{CliError, ExecuteOptions, Executor, IOMode, Printer};

pub async fn sam_build(pr: &Printer, ex: &Executor, project_dir: &Path) -> Result<(), CliError> {
    pr.info("Building with SAM...");
    let mut args = vec!["build"];
    let build_dir_override = std::env::var("CARGO_LAMBDA_BUILD_DIR");
    if let Ok(ref build_dir) = build_dir_override {
        pr.info(&format!("Using build dir: '{}'", build_dir));
        args.push("--build-dir");
        args.push(build_dir);
    }
    ex.execute_with_options(
        "sam",
        &args,
        IOMode::Attach,
        ExecuteOptions {
            dir: Some(project_dir),
            ..Default::default()
        },
    )?;
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
