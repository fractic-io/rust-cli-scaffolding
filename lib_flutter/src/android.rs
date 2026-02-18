use lib_core::{
    define_cli_error, deterministic_number_from_string, CliError, CriticalError, Executor, IOMode,
    Printer,
};

define_cli_error!(
    FailedToSetAvdOrientation,
    "Failed to set the orientation of the AVD '{avd_name}' to '{orientation}': {details}.",
    { avd_name: &str, orientation: &str, details: &str }
);

#[derive(Debug, Clone, Copy)]
pub enum Orientation {
    Portrait,
    Landscape,
}

impl Orientation {
    fn as_str(&self) -> &'static str {
        match self {
            Orientation::Portrait => "portrait",
            Orientation::Landscape => "landscape",
        }
    }
}

define_cli_error!(
    AndroidSystemImageMissing,
    "The required system image '{system_image}' is not installed, and failed to install with sdkmanager. If running with Nix, installing at runtime is not possible due to the read-only filesystem (instead, the system image should be declared in the flakes.nix).",
    { system_image: &str }
);

pub async fn create_android_emulator_if_not_exists(
    pr: &Printer,
    ex: &Executor,
    avd_id: &str,
    avd_image: &str,
    orientation: Orientation,
) -> Result<(), CliError> {
    let avd_exists = ex
        .execute("avdmanager", &["list", "avd"], IOMode::Silent)
        .await?
        .split("\n")
        .any(|line| line.trim() == format!("Name: {}", avd_id));

    if !avd_exists {
        pr.info(&format!(
            "Ensuring system image '{}' is installed...",
            avd_image
        ));
        ex.execute("sdkmanager", &[avd_image], IOMode::StreamOutput)
            .await
            .map_err(|e| AndroidSystemImageMissing::with_debug(avd_image, &e))?;

        pr.info(&format!(
            "Creating AVD '{}' with image '{}'...",
            avd_id, avd_image
        ));

        // Create the AVD.
        ex.execute(
            "avdmanager",
            &[
                "create", "avd", "-n", avd_id, "-d", avd_id, "-k", avd_image, "--force",
            ],
            IOMode::StreamOutput,
        )
        .await?;
        set_avd_orientation(avd_id, orientation)?;
    } else {
        pr.info(&format!("AVD '{}' already exists.", avd_id));
    }
    Ok(())
}

pub async fn start_android_emulator(
    pr: &Printer,
    ex: &mut Executor,
    avd_id: &str,
) -> Result<String, CliError> {
    // Start the emulator
    pr.info(&format!("Starting emulator for AVD '{}'...", avd_id));

    // Generate ~unique port number deterministically based on the avd_id. This
    // allows us to uniquely identify the emulator in adb commands, since the
    // emulator name is based on port.
    let port = port_number_from_avd_id(avd_id);
    let adb_id = format!("emulator-{}", port);

    // NOTES:
    //   - This executable needs to be $ANDROID_HOME/emulator/emulator, not
    //   $ANDROID_HOME/tools/emulator. This should be the default in a modern
    //   setup, especially when using Nix.
    //   - Errors are by default not output to stderr, so they are not displayed
    //   by execute_background. So we redirect with grep.
    let cmd = format!(
        "emulator -no-snapshot -wipe-data -no-window -no-audio -port {} -avd {} -delay-adb 2>&1 | tee >(grep 'ERROR' >&2)",
        port, avd_id
    );
    ex.execute_background("bash", &["-c", &cmd], None).await?;

    // Wait for the emulator to start.
    pr.info("Waiting for device to boot...");
    ex.execute(
        "adb",
        &[
            "-s",
            &adb_id,
            "wait-for-device",
            "shell",
            "while [[ -z $(getprop sys.boot_completed) ]]; do sleep 1; done",
        ],
        IOMode::StreamOutput,
    )
    .await?;

    // Wait 5s.
    pr.info("Waiting extra 5s...");
    std::thread::sleep(std::time::Duration::from_secs(5));

    Ok(adb_id)
}

pub async fn kill_android_emulator(
    pr: &Printer,
    ex: &Executor,
    adb_id: String,
) -> Result<(), CliError> {
    // Stop the emulator.
    pr.info(&format!("Stopping emulator '{}'...", adb_id));
    ex.execute(
        "adb",
        &["-s", &adb_id, "shell", "reboot", "-p"],
        IOMode::Silent,
    )
    .await?;

    // Wait 5s.
    pr.info("Waiting extra 5s...");
    std::thread::sleep(std::time::Duration::from_secs(5));

    Ok(())
}

// Helper functions.
// -----------------------------------------

fn port_number_from_avd_id(avd_id: &str) -> u32 {
    deterministic_number_from_string(avd_id, 5600, 5800)
}

fn set_avd_orientation(avd_name: &str, orientation: Orientation) -> Result<(), CliError> {
    let android_home_path = std::env::var("ANDROID_USER_HOME")
        .or_else(|_| std::env::var("HOME").map(|home| format!("{}/.android", home)))
        .map_err(|_| CriticalError::new("neither $ANDROID_USER_HOME nor $HOME is set"))?;

    // Compute the path to the AVD configuration file.
    let config_path = std::path::PathBuf::from(format!(
        "{}/avd/{}.avd/config.ini",
        android_home_path, avd_name
    ));

    if !config_path.exists() {
        return Err(FailedToSetAvdOrientation::with_debug(
            avd_name,
            orientation.as_str(),
            "config file not found",
            &config_path,
        ));
    }

    // Read and modify the config.ini file.
    let config_str = std::fs::read_to_string(&config_path).map_err(|e| {
        FailedToSetAvdOrientation::with_debug(
            avd_name,
            orientation.as_str(),
            "failed to read config",
            &e,
        )
    })?;

    let mut lines: Vec<String> = config_str.lines().map(|l| l.to_string()).collect();

    let orientation_str = orientation.as_str();

    // Find or insert hw.initialOrientation.
    let mut found_orientation = false;
    for line in lines.iter_mut() {
        if line.starts_with("hw.initialOrientation=") {
            *line = format!("hw.initialOrientation={}", orientation_str);
            found_orientation = true;
            break;
        }
    }

    if !found_orientation {
        lines.push(format!("hw.initialOrientation={}", orientation_str));
    }

    // Parse the current width/height.
    let mut width_val: Option<i32> = None;
    let mut height_val: Option<i32> = None;

    for line in &lines {
        if line.starts_with("hw.lcd.width=") {
            let val_str = line.trim_start_matches("hw.lcd.width=");
            width_val = val_str.parse().ok();
        } else if line.starts_with("hw.lcd.height=") {
            let val_str = line.trim_start_matches("hw.lcd.height=");
            height_val = val_str.parse().ok();
        }
    }

    // If we couldn't find these values, consider returning an error or setting defaults.
    let mut width = width_val.ok_or_else(|| {
        FailedToSetAvdOrientation::with_debug(
            avd_name,
            orientation.as_str(),
            "hw.lcd.width not found or not an integer",
            &config_path,
        )
    })?;
    let mut height = height_val.ok_or_else(|| {
        FailedToSetAvdOrientation::with_debug(
            avd_name,
            orientation.as_str(),
            "hw.lcd.height not found or not an integer",
            &config_path,
        )
    })?;

    // Adjust dimensions based on orientation.
    match orientation {
        Orientation::Portrait => {
            // Ensure height > width.
            if width > height {
                std::mem::swap(&mut width, &mut height);
            }
        }
        Orientation::Landscape => {
            // Ensure width > height.
            if height > width {
                std::mem::swap(&mut width, &mut height);
            }
        }
    }

    // Update the lines for hw.lcd.width and hw.lcd.height.
    let mut updated_width = false;
    let mut updated_height = false;
    for line in lines.iter_mut() {
        if line.starts_with("hw.lcd.width=") {
            *line = format!("hw.lcd.width={}", width);
            updated_width = true;
        } else if line.starts_with("hw.lcd.height=") {
            *line = format!("hw.lcd.height={}", height);
            updated_height = true;
        }
    }

    // If not found, push them (just in case).
    if !updated_width {
        lines.push(format!("hw.lcd.width={}", width));
    }
    if !updated_height {
        lines.push(format!("hw.lcd.height={}", height));
    }

    let new_config = lines.join("\n");

    std::fs::write(&config_path, new_config).map_err(|e| {
        FailedToSetAvdOrientation::with_debug(
            avd_name,
            orientation.as_str(),
            "failed to write changes",
            &e,
        )
    })?;

    Ok(())
}
