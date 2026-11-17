use std::{io::Write as _, time::Duration};

use inquire::InquireError;
use tokio::{io::AsyncBufReadExt as _, time::timeout};

use crate::{define_cli_error, CliError, IOError, UserCancelled};

define_cli_error!(PromptError, "Prompt failed.");

/// The error for a failed `inquire` prompt: [`UserCancelled`] for Esc or
/// Ctrl-C, `failure(&error)` for anything else.
pub fn prompt_failure(
    error: InquireError,
    failure: impl FnOnce(&InquireError) -> CliError,
) -> CliError {
    match error {
        InquireError::OperationCanceled | InquireError::OperationInterrupted => {
            UserCancelled::new()
        }
        error => failure(&error),
    }
}

pub fn confirm() -> Result<(), CliError> {
    match inquire::Confirm::new("Are you sure?").prompt() {
        Ok(true) => Ok(()),
        Ok(false) => Err(UserCancelled::new()),
        Err(error) => Err(prompt_failure(error, |e| PromptError::with_debug(e))),
    }
}

pub fn yes_no(prompt: &str) -> Result<bool, CliError> {
    inquire::Confirm::new(prompt)
        .prompt()
        .map_err(|error| prompt_failure(error, |e| PromptError::with_debug(e)))
}

pub fn continue_after_enter(message: Option<&str>) -> Result<(), CliError> {
    print!("{}", message.unwrap_or("Press Enter to continue..."));
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
pub async fn continue_after_enter_with_timeout(
    t: Duration,
    message: Option<&str>,
) -> Result<bool, CliError> {
    print!("{}", message.unwrap_or("Press Enter to continue..."));
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
        .map_err(|error| prompt_failure(error, |e| PromptError::with_debug(e)))
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
        .map_err(|error| prompt_failure(error, |e| PromptError::with_debug(e)))
}

pub fn ask_optional(prompt: &'static str) -> Result<Option<String>, CliError> {
    inquire::Text::new(prompt)
        .prompt()
        .map_err(|error| prompt_failure(error, |e| PromptError::with_debug(e)))
        .map(|x| if x.is_empty() { None } else { Some(x) })
}

// Tests.
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn esc_and_ctrl_c_are_cancels_and_other_failures_are_not() {
        for error in [
            InquireError::OperationCanceled,
            InquireError::OperationInterrupted,
        ] {
            assert!(prompt_failure(error, |e| PromptError::with_debug(e)).is_cancel());
        }
        let failure = prompt_failure(InquireError::NotTTY, |e| PromptError::with_debug(e));
        assert!(!failure.is_cancel());
        assert_eq!(failure.message(), "Prompt failed.");
    }
}
