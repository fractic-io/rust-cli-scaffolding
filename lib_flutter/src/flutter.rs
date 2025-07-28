use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use lib_core::{define_cli_error, CliError, ExecuteOptions, Executor, IOMode, Printer};

define_cli_error!(
    FlutterBuildTypeDoesntSupportInstall,
    "The build type {build_type:?} doesn't support installation.",
    { build_type: &BuildFor }
);
define_cli_error!(
    FlutterUnexpectedBuildOutputPath,
    "Expected build to be at '{expected}', but it could not be found.",
    { expected: &str }
);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildFor {
    Android,
    AndroidPublish,
    Ios,
    Web { base_href: String },
}

#[derive(Debug, Clone, Copy)]
pub enum BuildType {
    Debug,
    Profile,
    Release,
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

    ex.execute_with_options(
        "flutter",
        &args,
        IOMode::StreamOutput,
        ExecuteOptions {
            dir: Some(dir),
            ..Default::default()
        },
    )?;

    Ok(())
}

/// Returns the path to the generated output.
pub fn flutter_build(
    pr: &Printer,
    ex: &Executor,
    dir: &Path,
    os: BuildFor,
    build_type: BuildType,
    flavor: Option<&str>,
) -> Result<PathBuf, CliError> {
    let mut args = vec!["build"];
    let flavor_str = flavor.map_or("".to_string(), |f| format!("-{f}"));
    let output_path_suffix = match build_type {
        BuildType::Debug => "debug",
        BuildType::Profile => "profile",
        BuildType::Release => "release",
    };
    let output_path = match os {
        BuildFor::Android => {
            pr.info("Building Android apk...");
            args.push("apk");
            format!("build/app/outputs/flutter-apk/app{flavor_str}-{output_path_suffix}.apk")
        }
        BuildFor::AndroidPublish => {
            pr.info("Building Android app bundle...");
            args.push("appbundle");
            format!("build/app/outputs/bundle/release/app{flavor_str}-{output_path_suffix}.aab")
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
    match build_type {
        BuildType::Debug => args.push("--debug"),
        BuildType::Profile => args.push("--profile"),
        BuildType::Release => args.push("--release"),
    }
    if let Some(flavor) = flavor {
        args.extend(&["--flavor", flavor]);
    }
    if os == BuildFor::Android || os == BuildFor::AndroidPublish {
        // Flutter still temporarily supports Android x86 builds, but this is
        // not necessary except for very rare cases, so build for arm only:
        args.push("--target-platform");
        args.push("android-arm,android-arm64");
    }
    ex.execute_with_options(
        "flutter",
        &args,
        IOMode::StreamOutput,
        ExecuteOptions {
            dir: Some(dir),
            ..Default::default()
        },
    )?;
    fs::canonicalize(dir.join(&output_path))
        .map_err(|e| FlutterUnexpectedBuildOutputPath::with_debug(&output_path, &e))
}

pub fn flutter_install(
    pr: &Printer,
    ex: &Executor,
    dir: &Path,
    os: BuildFor,
    build_type: BuildType,
    flavor: Option<&str>,
) -> Result<(), CliError> {
    let output_path = flutter_build(pr, ex, dir, os.clone(), build_type, flavor)?;

    pr.info(&format!(
        "Installing '{}'...",
        output_path.to_string_lossy()
    ));
    let mut install_args = vec!["install"];
    if let Some(flavor) = flavor {
        install_args.extend(&["--flavor", flavor]);
    }
    match build_type {
        BuildType::Debug => install_args.push("--debug"),
        BuildType::Profile => install_args.push("--profile"),
        BuildType::Release => install_args.push("--release"),
    }
    match os {
        BuildFor::Android => {
            // First try directly installing with adb (to do "streamed install"
            // if the app is already exists), but fall back to flutter install.
            ex.execute(
                "adb",
                &["install", "-r", &output_path.to_string_lossy()],
                IOMode::StreamOutput,
            )
            .or_else(|_| {
                ex.execute_with_options(
                    "flutter",
                    &install_args,
                    IOMode::StreamOutput,
                    ExecuteOptions {
                        dir: Some(dir),
                        ..Default::default()
                    },
                )
            })?;
        }
        BuildFor::Ios => {
            ex.execute_with_options(
                "flutter",
                &install_args,
                IOMode::StreamOutput,
                ExecuteOptions {
                    dir: Some(dir),
                    ..Default::default()
                },
            )?;
        }
        _ => return Err(FlutterBuildTypeDoesntSupportInstall::new(&os)),
    }

    Ok(())
}
