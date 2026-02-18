use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use lib_core::{
    define_cli_error, with_tmp_edits_to_file, with_written_to_tmp_file_at_path, CliError,
    ExecuteOptions, Executor, IOError, IOMode, Printer,
};
use regex::Regex;

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
define_cli_error!(
    InvalidIosProvisioningProfile,
    "Invalid iOS provisioning profile: {details}.",
    { details: &str }
);
define_cli_error!(
    UnknownIosBundleName,
    "Could not extract iOS bundle name from ios/Runner/Info.plist: {details}.",
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
    pub xcode_signing_override: Option<XcodeSigningOverride<'a>>,
}

#[derive(Debug, Clone)]
pub struct XcodeSigningOverride<'a> {
    pub team_id: &'a str,
    pub bundle_id: &'a str,
    pub provisioning_profile: &'a [u8],
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
    // Constants.
    // ==
    let options = options.unwrap_or_default();
    let flavor_str = options.flavor.map_or("".to_string(), |f| format!("-{f}"));
    let output_path_suffix = match build_type {
        BuildType::Debug => "debug",
        BuildType::Profile => "profile",
        BuildType::Release => "release",
    };
    let export_options_dest = dir.join("ios").join("ExportOptions.plist");
    let export_options_dest_str = export_options_dest.to_string_lossy().to_string();

    // Prepare flutter build arguments.
    // ==
    let mut args = vec!["build"];
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
            if options.xcode_signing_override.is_some() {
                // (Export method will be dictated by the plist we generate later.)
                args.extend(["--export-options-plist", &export_options_dest_str]);
            } else {
                args.extend(["--export-method", "development"]);
            }
            format!("build/ios/ipa/{}.ipa", extract_ios_bundle_name(dir)?)
        }
        BuildFor::IosPublish => {
            pr.info("Building iOS IPA (app-store)...");
            args.push("ipa");
            if options.xcode_signing_override.is_some() {
                // (Export method will be dictated by the plist we generate later.)
                args.extend(["--export-options-plist", &export_options_dest_str]);
            } else {
                args.extend(["--export-method", "app-store"]);
            }
            format!("build/ios/ipa/{}.ipa", extract_ios_bundle_name(dir)?)
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

    // Prepare main build function, and add optional override wrappers for iOS.
    // ==
    let build_fn = || {
        ex.execute_with_options_sync(
            "flutter",
            &args,
            IOMode::StreamOutput,
            ExecuteOptions {
                dir: Some(dir),
                ..Default::default()
            },
        )
    };
    if let Some(xc) = options.xcode_signing_override {
        let for_app_store = match os {
            BuildFor::Ios => false,
            BuildFor::IosPublish => true,
            _ => {
                return Err(FlutterInvalidBuildOptions::new(
                    "XcodeSigningOverride is only supported for iOS builds",
                ))
            }
        };
        // Wrapper 1: Write provisioning profile.
        // --
        let home_dir = std::env::var("HOME").map_err(|e| IOError::with_debug(&e))?;
        let profiles_dir = PathBuf::from(&home_dir)
            .join("Library")
            .join("MobileDevice")
            .join("Provisioning Profiles");
        std::fs::create_dir_all(&profiles_dir).map_err(|e| IOError::with_debug(&e))?;
        let profile_dest = profiles_dir.join("tmp_cli_build.mobileprovision");
        let profile_uuid = extract_profile_uuid(xc.provisioning_profile)?;
        with_written_to_tmp_file_at_path(pr, xc.provisioning_profile, &profile_dest, || {
            // Wrapper 2: Override Xcode config (used by archive step).
            // --
            let xcconfig_path = dir.join("ios").join("Flutter").join(match build_type {
                BuildType::Debug => "Debug.xcconfig",
                BuildType::Profile | BuildType::Release => "Release.xcconfig",
            });
            let xconfig_patch_fn = |original: String| {
                let mut lines: Vec<String> =
                    original.lines().map(|s| s.to_string() + "\n").collect();
                lines = override_xcconfig_key(lines, "DEVELOPMENT_TEAM", xc.team_id);
                lines = override_xcconfig_key(lines, "CODE_SIGN_STYLE", "Manual");
                lines =
                    override_xcconfig_key(lines, "PROVISIONING_PROFILE_SPECIFIER", &profile_uuid);
                lines = override_xcconfig_key(
                    lines,
                    "CODE_SIGN_IDENTITY",
                    match for_app_store {
                        true => "Apple Distribution",
                        false => "Apple Development",
                    },
                );
                lines.into_iter().collect()
            };
            with_tmp_edits_to_file(pr, &xcconfig_path, xconfig_patch_fn, || {
                // Wrapper 3: Override export options (used by export step).
                // --
                let export_plist_path = dir.join("ios").join("ExportOptions.plist");
                let export_plist_content = build_export_plist_content(
                    match for_app_store {
                        true => "app-store",
                        false => "development",
                    },
                    xc.bundle_id,
                    &profile_uuid,
                    xc.team_id,
                );
                with_written_to_tmp_file_at_path(
                    pr,
                    export_plist_content,
                    &export_plist_path,
                    || build_fn(),
                )
            })
        })?;
    } else {
        build_fn()?;
    }

    // Validate and return path to completed build.
    // ==
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
            ex.execute_sync(
                "adb",
                &["install", "-r", &output_path.to_string_lossy()],
                IOMode::StreamOutput,
            )
            .or_else(|_| {
                ex.execute_with_options_sync(
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
            ex.execute_sync(
                "ideviceinstaller",
                &["install", &output_path.to_string_lossy()],
                IOMode::StreamOutput,
            )?;
        }
        _ => return Err(FlutterBuildTypeDoesntSupportInstall::new(&os)),
    }

    Ok(())
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

    ex.execute_with_options_sync(
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

// Helpers.
// ---------------------------------------------------------------------------

fn override_xcconfig_key(mut lines: Vec<String>, key: &str, value: &str) -> Vec<String> {
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

fn build_export_plist_content(
    method: &str,
    bundle_id: &str,
    profile_uuid: &str,
    team_id: &str,
) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>method</key>
    <string>{method}</string>
    <key>compileBitcode</key>
    <false/>
    <key>manageAppVersionAndBuildNumber</key>
    <false/>
    <key>signingStyle</key>
    <string>manual</string>
    <key>provisioningProfiles</key>
    <dict>
        <key>{bundle_id}</key>
        <string>{profile_uuid}</string>
    </dict>
    <key>stripSwiftSymbols</key>
    <true/>
    <key>teamID</key>
    <string>{team_id}</string>
    <key>thinning</key>
    <string>&lt;none&gt;</string>
</dict>
</plist>
"#
    )
}

fn extract_profile_uuid(profile_bytes: &[u8]) -> Result<String, CliError> {
    let bytes_str = String::from_utf8_lossy(profile_bytes);

    // Try to parse 'UUID' key.
    let re = Regex::new(r#"<key>UUID</key>\s*<string>([^<]+)</string>"#)
        .map_err(|e| InvalidIosProvisioningProfile::with_debug("could not compile regex", &e))?;

    if let Some(caps) = re.captures(&bytes_str) {
        if let Some(name) = caps.get(1) {
            return Ok(name.as_str().to_string());
        }
    }
    Err(InvalidIosProvisioningProfile::new(
        "could not find 'UUID' key",
    ))
}

fn extract_ios_bundle_name(dir: &Path) -> Result<String, CliError> {
    let plist_path = dir.join("ios").join("Runner").join("Info.plist");
    let contents = fs::read_to_string(&plist_path)
        .map_err(|e| UnknownIosBundleName::with_debug("failed to read file", &e))?;

    // Try to parse 'CFBundleName' key.
    let re = Regex::new(r#"<key>CFBundleName</key>\s*<string>([^<]+)</string>"#)
        .map_err(|e| UnknownIosBundleName::with_debug("could not compile regex", &e))?;

    if let Some(caps) = re.captures(&contents) {
        if let Some(name) = caps.get(1) {
            return Ok(name.as_str().to_string());
        }
    }
    Err(UnknownIosBundleName::new(
        "could not find 'CFBundleName' key",
    ))
}
