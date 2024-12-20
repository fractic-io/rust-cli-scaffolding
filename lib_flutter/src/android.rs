use std::env;

use lib_core::{
    define_cli_error, deterministic_number_from_string, CliError, CriticalError, Executor, IOMode,
    Printer,
};

define_cli_error!(
    AndroidSystemImageMissing,
    "The required system image '{system_image}' is not installed, and failed to install with sdkmanager. If running with Nix, installing at runtime is not possible due to the read-only filesystem (instead, the system image should be declared in the flakes.nix).",
    { system_image: &str }
);

pub fn create_android_emulator_if_not_exists(
    pr: &Printer,
    ex: &Executor,
    avd_id: &str,
    avd_image: &str,
) -> Result<(), CliError> {
    let avd_exists = ex
        .execute("avdmanager", &["list", "avd"], IOMode::Silent)?
        .split("\n")
        .any(|line| line.trim() == format!("Name: {}", avd_id));

    if !avd_exists {
        pr.info(&format!(
            "Ensuring system image '{}' is installed...",
            avd_image
        ));
        ex.execute("sdkmanager", &[avd_image], IOMode::StreamOutput)
            .map_err(|e| AndroidSystemImageMissing::with_debug(avd_image, &e))?;

        pr.info(&format!(
            "Creating AVD '{}' with image '{}'...",
            avd_id, avd_image
        ));

        // Create the AVD
        ex.execute(
            "avdmanager",
            &[
                "create", "avd", "-n", avd_id, "-d", avd_id, "-k", avd_image, "--force",
            ],
            IOMode::StreamOutput,
        )?;
    } else {
        pr.info(&format!("AVD '{}' already exists.", avd_id));
    }
    Ok(())
}

pub fn start_android_emulator(
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

    // NOTE: This executable needs to be $ANDROID_HOME/emulator/emulator, not
    // $ANDROID_HOME/tools/emulator. This should be the default in a modern
    // setup, especially when using Nix.
    ex.execute_background(
        &format!("emulator", android_home),
        &[
            "-no-snapshot",
            "-wipe-data",
            "-no-window",
            "-no-audio",
            "-port",
            &port.to_string(),
            "-avd",
            avd_id,
            "-delay-adb",
        ],
        None,
    )?;

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
    )?;

    // Wait 5s.
    pr.info("Waiting extra 5s...");
    std::thread::sleep(std::time::Duration::from_secs(5));

    Ok(adb_id)
}

pub fn kill_android_emulator(pr: &Printer, ex: &Executor, adb_id: String) -> Result<(), CliError> {
    // Stop the emulator
    pr.info(&format!("Stopping emulator '{}'...", adb_id));
    ex.execute(
        "adb",
        &["-s", &adb_id, "shell", "reboot", "-p"],
        IOMode::Silent,
    )?;

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
