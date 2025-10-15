use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use lib_core::{
    define_cli_error, with_tmp_edits_to_file, CliError, ExecuteOptions, Executor, IOMode, Printer,
};

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
define_cli_error!(
    FlutterInvalidBuildOptions,
    "Invalid build options: {details}.",
    { details: &str }
);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildFor {
    Android,
    AndroidPublish,
    Ios,
    IosPublish,
    Web { base_href: String },
}

#[derive(Debug, Clone, Copy)]
pub enum BuildType {
    Debug,
    Profile,
    Release,
}

#[derive(Debug, Clone, Default)]
pub struct BuildOptions<'a> {
    pub flavor: Option<&'a str>,
    pub export_options_plist: Option<&'a Path>,
    pub xcode_config: Option<XcodeConfig<'a>>, // iOS-only: temporary overrides for xcconfig
}

#[derive(Debug, Clone)]
pub struct XcodeConfig<'a> {
    pub team_id: &'a str,
    pub code_sign_identity: &'a str,
    pub provisioning_profile_specifier: &'a str,
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
    options: Option<BuildOptions>,
) -> Result<PathBuf, CliError> {
    let options = options.unwrap_or_default();
    let mut args = vec!["build"];
    let flavor_str = options.flavor.map_or("".to_string(), |f| format!("-{f}"));
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
            let release_dir = match options.flavor {
                None => "release".to_string(),
                Some(f) => format!("{f}Release"),
            };
            format!(
                "build/app/outputs/bundle/{release_dir}/app{flavor_str}-{output_path_suffix}.aab"
            )
        }
        BuildFor::Ios => {
            pr.info("Building iOS IPA (development)...");
            args.push("ipa");
            if let Some(plist) = options.export_options_plist {
                args.push("--export-options-plist");
                args.push(plist.to_str().ok_or_else(|| {
                    FlutterInvalidBuildOptions::new("export options plist path is not valid")
                })?);
            } else {
                args.extend(["--export-method", "development"]);
            }
            "build/ios/ipa/Runner.ipa".to_string()
        }
        BuildFor::IosPublish => {
            pr.info("Building iOS IPA (app-store)...");
            args.push("ipa");
            if let Some(plist) = options.export_options_plist {
                args.push("--export-options-plist");
                args.push(plist.to_str().ok_or_else(|| {
                    FlutterInvalidBuildOptions::new("export options plist path is not valid")
                })?);
            } else {
                args.extend(["--export-method", "app-store"]);
            }
            "build/ios/ipa/Runner.ipa".to_string()
        }
        BuildFor::Web { ref base_href } => {
            pr.info("Building web app...");
            args.extend(["web", "--base-href", base_href]);
            "build/web".to_string()
        }
    };
    match build_type {
        BuildType::Debug => args.push("--debug"),
        BuildType::Profile => args.push("--profile"),
        BuildType::Release => args.push("--release"),
    }
    if let Some(flavor) = options.flavor {
        args.extend(["--flavor", flavor]);
    }
    if os == BuildFor::Android || os == BuildFor::AndroidPublish {
        // Flutter still temporarily supports Android x86 builds, but this is
        // not necessary except for very rare cases, so build for arm only:
        args.push("--target-platform");
        args.push("android-arm,android-arm64");
    }
    let run_flutter_build = || {
        ex.execute_with_options(
            "flutter",
            &args,
            IOMode::StreamOutput,
            ExecuteOptions {
                dir: Some(dir),
                ..Default::default()
            },
        )
    };
    if (os == BuildFor::Ios || os == BuildFor::IosPublish) && options.xcode_config.is_some() {
        let xc = options.xcode_config.as_ref().unwrap();
        let xcconfig_file_name = match build_type {
            BuildType::Debug => "Debug.xcconfig",
            BuildType::Profile | BuildType::Release => "Release.xcconfig",
        };
        let xcconfig_path = dir.join("ios").join("Flutter").join(xcconfig_file_name);
        with_tmp_edits_to_file(
            pr,
            &xcconfig_path,
            |original| {
                let mut lines: Vec<String> =
                    original.lines().map(|s| s.to_string() + "\n").collect();
                lines = override_config_key(lines, "DEVELOPMENT_TEAM", xc.team_id);
                lines = override_config_key(lines, "CODE_SIGN_STYLE", "Manual");
                lines = override_config_key(lines, "CODE_SIGN_IDENTITY", xc.code_sign_identity);
                lines = override_config_key(
                    lines,
                    "PROVISIONING_PROFILE_SPECIFIER",
                    xc.provisioning_profile_specifier,
                );
                lines.into_iter().collect()
            },
            run_flutter_build,
        )?;
    } else {
        run_flutter_build()?;
    }
    fs::canonicalize(dir.join(&output_path))
        .map_err(|e| FlutterUnexpectedBuildOutputPath::with_debug(&output_path, &e))
}

pub fn flutter_install(
    pr: &Printer,
    ex: &Executor,
    dir: &Path,
    os: BuildFor,
    build_type: BuildType,
    options: Option<BuildOptions>,
) -> Result<(), CliError> {
    let output_path = flutter_build(pr, ex, dir, os.clone(), build_type, options.clone())?;

    pr.info(&format!(
        "Installing '{}'...",
        output_path.to_string_lossy()
    ));
    let options = options.unwrap_or_default();
    let mut install_args = vec!["install"];
    if let Some(flavor) = options.flavor {
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
        BuildFor::Ios | BuildFor::IosPublish => {
            // Install with linux-friendly ideviceinstaller.
            ex.execute(
                "ideviceinstaller",
                &["install", &output_path.to_string_lossy()],
                IOMode::StreamOutput,
            )?;
        }
        _ => return Err(FlutterBuildTypeDoesntSupportInstall::new(&os)),
    }

    Ok(())
}

// Helpers.
// ---------------------------------------------------------------------------

fn override_config_key(mut lines: Vec<String>, key: &str, value: &str) -> Vec<String> {
    let mut replaced = false;
    let key_len = key.len();
    for line in lines.iter_mut() {
        let trimmed_start = line.trim_start();
        if trimmed_start.starts_with(key) {
            let rest = &trimmed_start[key_len..];
            if rest.trim_start().starts_with('=') {
                *line = format!("{}={}", key, value);
                replaced = true;
            }
        }
    }
    if !replaced {
        if let Some(last) = lines.last() {
            if !last.ends_with('\n') {
                lines.push(String::from("\n"));
            }
        }
        lines.push(format!("{}={}\n", key, value));
    }
    lines
}
