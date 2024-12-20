use std::{collections::HashMap, path::Path};

use lib_core::{ln_s, mkdir_p, rm_rf, CliError, ExecuteOptions, Executor, IOMode, Printer};

pub async fn sam_build(
    pr: &Printer,
    ex: &Executor,
    project_dir: &Path,
    debug: bool,
) -> Result<(), CliError> {
    pr.info("Building with SAM...");

    let mut env = Vec::new();

    if let Ok(target_dir) = std::env::var("CARGO_LAMBDA_TARGET_DIR") {
        pr.info(&format!("Using custom target dir: '{}'.", target_dir));

        // It seems SAM isn't fully compatible with a custom CARGO_TARGET_DIR,
        // so we need to symlink the expected '/target' directory to our custom
        // location.
        let expected_target_dir = project_dir.join("code").join("target");
        rm_rf(&expected_target_dir)?;
        mkdir_p(&target_dir)?;
        ln_s(&target_dir, &expected_target_dir)?;
        env.push(("CARGO_TARGET_DIR".to_string(), target_dir));
    };

    if debug {
        env.push(("SAM_BUILD_MODE".to_string(), "debug".to_string()));
    }

    ex.execute_with_options(
        "sam",
        &["build"],
        IOMode::Attach,
        ExecuteOptions {
            dir: Some(project_dir),
            env: Some(env),
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
