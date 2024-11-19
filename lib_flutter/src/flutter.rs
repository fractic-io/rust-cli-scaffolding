use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use lib_core::{define_cli_error, CliError, Executor, IOMode, Printer};

define_cli_error!(
    FlutterBuildTypeDoesntSupportInstall,
    "The build type {build_type:?} doesn't support installation.",
    { build_type: &BuildFor }
);

#[derive(Debug, Clone)]
pub enum BuildFor {
    Android,
    AndroidPublish,
    Ios,
    Web { base_href: String },
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

/// Returns the path to the generated output.
pub fn flutter_build_release(
    pr: &Printer,
    ex: &Executor,
    dir: &Path,
    os: BuildFor,
    flavor: Option<&str>,
) -> Result<PathBuf, CliError> {
    let mut args = vec!["build"];
    let flavor_str = flavor.map_or("".to_string(), |f| format!("-{f}"));
    let output_path = match os {
        BuildFor::Android => {
            pr.info("Building Android apk...");
            args.push("apk");
            format!("build/app/outputs/flutter-apk/app${flavor_str}-release.apk")
        }
        BuildFor::AndroidPublish => {
            pr.info("Building Android app bundle...");
            args.push("appbundle");
            format!("build/app/outputs/bundle/release/app${flavor_str}-release.aab")
        }
        BuildFor::Ios => {
            pr.info("Building iOS app...");
            args.push("ios");
            "build/ios/iphoneos/Runner.app".to_string()
        }
        BuildFor::Web { ref base_href } => {
            pr.info("Building web app...");
            args.extend(&["web", "--base-href", base_href]);
            "build/web".to_string()
        }
    };
    args.push("--release");
    if let Some(flavor) = flavor {
        args.extend(&["--flavor", flavor]);
    }
    ex.execute("flutter", &args, Some(dir), IOMode::StreamOutput)?;
    Ok(dir.join(output_path))
}

pub fn flutter_install(
    pr: &Printer,
    ex: &Executor,
    dir: &Path,
    os: BuildFor,
    flavor: Option<&str>,
) -> Result<(), CliError> {
    let output_path = flutter_build_release(pr, ex, dir, os.clone(), flavor)?;

    pr.info("Installing...");
    let mut install_args = vec!["install"];
    if let Some(flavor) = flavor {
        install_args.extend(&["--flavor", flavor]);
    }
    match os {
        BuildFor::Android => {
            // First try directly installing with adb (to do "streamed install"
            // if the app is already exists), but fall back to flutter install.
            ex.execute(
                "adb",
                &["install", "-r", &output_path.to_string_lossy()],
                Some(dir),
                IOMode::StreamOutput,
            )
            .or_else(|_| ex.execute("flutter", &install_args, Some(dir), IOMode::StreamOutput))?;
        }
        BuildFor::Ios => {
            ex.execute("flutter", &install_args, Some(dir), IOMode::StreamOutput)?;
        }
        _ => return Err(FlutterBuildTypeDoesntSupportInstall::new(&os)),
    }

    Ok(())
}
