use std::{io::Write as _, time::Duration};

use tokio::{io::AsyncBufReadExt as _, time::timeout};

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
    print!("Press Enter to continue...");
    std::io::stdout()
        .flush()
        .map_err(|e| IOError::with_debug(&e))?;
    let mut buffer = String::new();
    std::io::stdin()
        .read_line(&mut buffer)
        .map_err(|e| IOError::with_debug(&e))?;
    Ok(())
}

// Returns true if the user pressed Enter, false if the timeout expired.
pub async fn continue_after_enter_with_timeout(t: Duration) -> Result<bool, CliError> {
    print!("Press Enter to continue...");
    std::io::stdout()
        .flush()
        .map_err(|e| IOError::with_debug(&e))?;

    let mut stdin_reader = tokio::io::BufReader::new(tokio::io::stdin());
    let mut buffer = String::new();

    match timeout(t, stdin_reader.read_line(&mut buffer)).await {
        Ok(Ok(_bytes_read)) => {
            // The user pressed Enter (or typed something and pressed Enter).
            Ok(true)
        }
        Ok(Err(e)) => {
            // An I/O error occurred.
            Err(IOError::with_debug(&e))
        }
        Err(_elapsed) => {
            // The timeout expired.
            println!();
            Ok(false)
        }
    }
}

pub fn ask(prompt: &'static str) -> Result<String, CliError> {
    inquire::Text::new(prompt)
        .with_validator(|x: &str| {
            if x.is_empty() {
                Ok(inquire::validator::Validation::Invalid(
                    inquire::validator::ErrorMessage::Default,
                ))
            } else {
                Ok(inquire::validator::Validation::Valid)
            }
        })
        .prompt()
        .map_err(|e| UserCancelled::with_debug(&e))
}

pub fn ask_secure(prompt: &'static str) -> Result<String, CliError> {
    inquire::Password::new(prompt)
        .with_validator(|x: &str| {
            if x.is_empty() {
                Ok(inquire::validator::Validation::Invalid(
                    inquire::validator::ErrorMessage::Default,
                ))
            } else {
                Ok(inquire::validator::Validation::Valid)
            }
        })
        .prompt()
        .map_err(|e| UserCancelled::with_debug(&e))
}

pub fn ask_optional(prompt: &'static str) -> Result<Option<String>, CliError> {
    inquire::Text::new(prompt)
        .prompt()
        .map_err(|e| UserCancelled::with_debug(&e))
        .map(|x| if x.is_empty() { None } else { Some(x) })
}
