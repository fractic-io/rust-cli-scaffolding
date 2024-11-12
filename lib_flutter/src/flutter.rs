use std::{collections::HashMap, error::Error};

use lib_core::Tty;

pub fn run_flutter_integration_test(
    tty: &Tty,
    dir: &str,
    adb_id: &str,
    driver: &str,
    target: &str,
    flavor: Option<&str>,
    dart_define: Option<HashMap<&str, &str>>,
) -> Result<(), Box<dyn Error>> {
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

    tty.execute("flutter", &args, Some(dir), true)?;

    Ok(())
}
