use std::{collections::HashMap, path::Path};

use lib_core::{CliError, Executor, IOMode, Printer};

pub async fn sam_build(pr: &Printer, ex: &Executor, project_dir: &Path) -> Result<(), CliError> {
    pr.info("Building with SAM...");
    let args = vec!["build"];
    if let Ok(build_dir) = std::env::var("CARGO_LAMBDA_BUILD_DIR") {
        pr.info(&format!("Using build dir: '{}'", build_dir));
        args.push("--build-dir");
        args.push(&build_dir);
    }
    ex.execute("sam", &args, Some(project_dir), IOMode::Attach)?;
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
    ex.execute(
        "sam",
        &args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        Some(project_dir),
        IOMode::Attach,
    )?;
    Ok(())
}
