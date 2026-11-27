use crate::{define_cli_error, with_written_to_tmp_file, CliError};

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

    let edited = with_written_to_tmp_file(text.unwrap_or_default(), |path| {
        let mut vim = std::process::Command::new("vim");
        vim.arg(path);
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
        std::fs::read_to_string(path)
            .map_err(|e| VimError::with_debug("failed to read from temporary file", &e))
    })?;

    Ok(match edited.trim() {
        "" => None,
        x => Some(x.to_string()),
    })
}
