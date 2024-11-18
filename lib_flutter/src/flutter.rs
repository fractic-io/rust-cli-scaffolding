use std::{collections::HashMap, path::Path};

use lib_core::{CliError, Executor, Printer};

pub enum BuildFor {
    Android,
    Ios,
}

pub fn run_flutter_integration_test(
    ex: &Executor,
    dir: &Path,
    adb_id: &str,
    driver: &str,
    target: &str,
    flavor: Option<&str>,
    dart_define: Option<HashMap<&str, &str>>,
) -> Result<(), CliError> {
    let mut args = vec![
        "drive",
        "--profile",
        "--driver",
        driver,
        "--target",
        target,
        "--no-pub",
        "--device-id",
        adb_id,
        "--suppress-analytics",
    ];
    if let Some(flavor) = flavor {
        args.extend(&["--flavor", flavor]);
    }
    let setexprs: Vec<String> = dart_define
        .unwrap_or_default()
        .into_iter()
        .map(|(key, value)| format!("--dart-define={}={}", key, value))
        .collect();
    args.extend(setexprs.iter().map(|s| s.as_str()));

    ex.execute("flutter", &args, Some(dir), lib_core::IOMode::StreamOutput)?;

    Ok(())
}

pub fn flutter_build_release(
    pr: &Printer,
    ex: &Executor,
    app: &Path,
    os: BuildFor,
    flavor: Option<&str>,
) -> Result<(), CliError> {
    let mut args = vec!["build"];
    match os {
        BuildFor::Android => {
            pr.info("Building Android app bundle...");
            args.push("appbundle")
        }
        BuildFor::Ios => {
            pr.info("Building iOS app...");
            args.push("ios")
        }
    }
    args.push("--release");
    if let Some(flavor) = flavor {
        args.extend(&["--flavor", flavor]);
    }
    ex.execute("flutter", &args, Some(app), lib_core::IOMode::StreamOutput)?;
    Ok(())
}
