use crate::{define_cli_error, CliError, IOError};

define_cli_error!(UserCancelled, "User cancelled operation.");

pub fn confirm() -> Result<(), CliError> {
    match inquire::Confirm::new("Are you sure?").prompt() {
        Ok(true) => Ok(()),
        Ok(false) | Err(_) => Err(UserCancelled::new()),
    }
}

pub fn yes_no(prompt: &str) -> Result<bool, CliError> {
    match inquire::Confirm::new(prompt).prompt() {
        Ok(true) => Ok(true),
        Ok(false) => Ok(false),
        Err(_) => Err(UserCancelled::new()),
    }
}

pub fn continue_after_enter() -> Result<(), CliError> {
    println!("Press Enter to continue...");
    let mut buffer = String::new();
    std::io::stdin()
        .read_line(&mut buffer)
        .map_err(|e| IOError::with_debug(&e))?;
    Ok(())
}
