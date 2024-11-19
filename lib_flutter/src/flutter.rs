use std::{collections::HashMap, path::Path};

use lib_core::{CliError, Executor, IOMode, Printer};

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

    ex.execute("flutter", &args, Some(dir), IOMode::StreamOutput)?;

    Ok(())
}

pub fn flutter_build_release(
    pr: &Printer,
    ex: &Executor,
    dir: &Path,
    os: BuildFor,
    flavor: Option<&str>,
) -> Result<(), CliError> {
    let mut args = vec!["build"];
    match os {
        BuildFor::Android => {
            pr.info("Building Android app bundle...");
            args.push("appbundle");
        }
        BuildFor::Ios => {
            pr.info("Building iOS app...");
            args.push("ios");
        }
    }
    args.push("--release");
    if let Some(flavor) = flavor {
        args.extend(&["--flavor", flavor]);
    }
    ex.execute("flutter", &args, Some(dir), IOMode::StreamOutput)?;
    Ok(())
}

pub fn flutter_install(
    pr: &Printer,
    ex: &Executor,
    dir: &Path,
    os: BuildFor,
    flavor: Option<&str>,
) -> Result<(), CliError> {
    let mut build_args = vec!["build"];
    match os {
        BuildFor::Android => {
            pr.info("Building Android apk...");
            build_args.push("apk");
        }
        BuildFor::Ios => {
            pr.info("Building iOS app...");
            build_args.push("ios");
        }
    }
    build_args.push("--release");
    if let Some(flavor) = flavor {
        build_args.extend(&["--flavor", flavor]);
    }
    ex.execute(
        "flutter",
        &build_args,
        Some(dir),
        lib_core::IOMode::StreamOutput,
    )?;

    pr.info("Installing...");
    let mut install_args = vec!["install"];
    if let Some(flavor) = flavor {
        install_args.extend(&["--flavor", flavor]);
    }
    match os {
        BuildFor::Android => {
            // First try directly installing with adb (to do "streamed install"
            // if the app is already exists), but fall back to flutter install.
            let executable_path = match flavor {
                Some(f) => format!("build/app/outputs/flutter-apk/app-{f}-release.apk"),
                None => "build/app/outputs/flutter-apk/app-release.apk".to_string(),
            };
            ex.execute(
                "adb",
                &["install", "-r", &executable_path],
                Some(dir),
                IOMode::StreamOutput,
            )
            .or_else(|_| ex.execute("flutter", &install_args, Some(dir), IOMode::StreamOutput))?;
        }
        BuildFor::Ios => {
            ex.execute("flutter", &install_args, Some(dir), IOMode::StreamOutput)?;
        }
    }

    Ok(())
}
