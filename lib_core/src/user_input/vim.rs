use tempfile::NamedTempFile;

use crate::{define_cli_error, CliError};

define_cli_error!(VimError, "Vim error: {details}.", { details: &str });

pub fn vim(text: Option<String>) -> Result<Option<String>, CliError> {
    vim_custom(text, None, true, true)
}

pub fn vim_custom(
    text: Option<String>,
    extra_args: Option<Vec<&str>>,
    line_wrap: bool,
    start_insert_mode_if_empty: bool,
) -> Result<Option<String>, CliError> {
    let has_initial_text = text.is_some();

    // Write to temporary file.
    //
    // Using NamedTempFile is fairly secure, automatically deleting the
    // temporary file as soon as the object goes out of scope. It is, however,
    // important that the program does not have a hard exit (ex. sig fault or
    // process::exit) before the NamedTempFile is dropped.
    let temp_file = NamedTempFile::new()
        .map_err(|e| VimError::with_debug("failed to create temporary file", &e))?;
    if let Some(text) = &text {
        std::fs::write(&temp_file.path(), text)
            .map_err(|e| VimError::with_debug("failed to write to temporary file", &e))?;
    }

    // Open in vim.
    let mut vim = std::process::Command::new("vim");
    vim.arg(&temp_file.path());
    if start_insert_mode_if_empty && !has_initial_text {
        vim.arg("-c").arg("startinsert");
    }
    if let Some(extra_args) = extra_args {
        for arg in extra_args {
            vim.arg(arg);
        }
    }
    if line_wrap {
        vim.arg("+windo set wrap")
            .arg("+set textwidth=0")
            .arg("+set wrapmargin=0")
            .arg("+set linebreak")
            .arg("+noremap j gj")
            .arg("+noremap k gk");
    }
    if !vim
        .spawn()
        .map_err(|e| VimError::with_debug("failed to open Vim", &e))?
        .wait()
        .map_err(|e| VimError::with_debug("failed to wait for Vim to close", &e))?
        .success()
    {
        return Err(VimError::new("Vim exited with an error"));
    }

    // Read from temporary file.
    let text = std::fs::read_to_string(&temp_file)
        .map_err(|e| VimError::with_debug("failed to read from temporary file", &e))?;

    // NamedTempFiles are deleted automatically when they go out of scope, but
    // drop it explicitly just to be safe.
    drop(temp_file);

    Ok(match text.trim() {
        "" => None,
        x => Some(x.to_string()),
    })
}
