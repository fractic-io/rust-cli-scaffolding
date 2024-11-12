use std::{
    error::Error,
    hash::{DefaultHasher, Hash as _, Hasher as _},
};

use lib_core::Tty;

pub fn create_android_emulator_if_not_exists(
    tty: &Tty,
    avd_id: &str,
    avd_image: &str,
) -> Result<(), Box<dyn Error>> {
    let avd_exists = tty
        .execute("avdmanager", &["list", "avd"], None, false)?
        .split("\n")
        .any(|line| line.trim() == format!("Name: {}", avd_id));

    if !avd_exists {
        tty.info(&format!(
            "Ensuring system image '{}' is installed...",
            avd_image
        ));
        tty.execute("sdkmanager", &[avd_image], None, true)?;

        tty.info(&format!(
            "Creating AVD '{}' with image '{}'...",
            avd_id, avd_image
        ));
        // Create the AVD
        tty.execute(
            "avdmanager",
            &[
                "create", "avd", "-n", avd_id, "-d", avd_id, "-k", avd_image, "--force",
            ],
            None,
            true,
        )?;
    } else {
        tty.debug(&format!("AVD '{}' already exists.", avd_id));
    }
    Ok(())
}

pub fn start_android_emulator(tty: &mut Tty, avd_id: &str) -> Result<String, Box<dyn Error>> {
    // Start the emulator
    tty.info(&format!("Starting emulator for AVD '{}'...", avd_id));

    // Generate ~unique port number deterministically based on the avd_id. This
    // allows us to uniquely identify the emulator in adb commands, since the
    // emulator name is based on port.
    let port = port_number_from_avd_id(avd_id);
    let adb_id = format!("emulator-{}", port);

    // NOTE: This executable needs to be $ANDROID_HOME/emulator/emulator, not
    // $ANDROID_HOME/tools/emulator.
    tty.execute_background(
        "emulator",
        &[
            "-no-snapshot",
            "-wipe-data",
            "-no-window",
            "-port",
            &port.to_string(),
            "-avd",
            avd_id,
            "-delay-adb",
        ],
        None,
        false,
    )?;

    // Wait for the emulator to start.
    tty.info("Waiting for device to boot...");
    tty.execute(
        "adb",
        &[
            "-s",
            &adb_id,
            "wait-for-device",
            "shell",
            "while [[ -z $(getprop sys.boot_completed) ]]; do sleep 1; done",
        ],
        None,
        true,
    )?;

    // Wait 5s.
    tty.debug("Waiting extra 5s...");
    std::thread::sleep(std::time::Duration::from_secs(5));

    Ok(adb_id)
}

pub fn kill_android_emulator(tty: &Tty, adb_id: String) -> Result<(), Box<dyn Error>> {
    // Stop the emulator
    tty.info(&format!("Stopping emulator '{}'...", adb_id));
    tty.execute(
        "adb",
        &["-s", &adb_id, "shell", "reboot", "-p"],
        None,
        false,
    )?;

    // Wait 5s.
    tty.debug("Waiting extra 5s...");
    std::thread::sleep(std::time::Duration::from_secs(5));

    Ok(())
}

// Helper functions.
// -----------------------------------------

fn port_number_from_avd_id(avd_id: &str) -> u32 {
    deterministic_number_from_string(avd_id, 5600, 5800)
}

fn deterministic_number_from_string(input: &str, min: u32, max: u32) -> u32 {
    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    let hash_value = hasher.finish();

    // Scale the hash value to the range [min, max]
    min + (hash_value % (max - min + 1) as u64) as u32
}
