use std::path::Path;

use tempfile::NamedTempFile;

use crate::{define_cli_error, CliError, Printer};

use super::{ln_s, rm};

define_cli_error!(TemporaryFileError, "Temporary file error: {details}.", { details: &str });

/// Perform an operation with 'content' written to a temporary file.
///
/// This function uses NamedTempFile, which is fairly secure. It automatically
/// deletes the temporary file as soon as the object goes out of scope, even if
/// the function fails midway. It is, however, important that the program does
/// not have a hard exit (ex. sig fault or process::exit) while executing 'op'.
pub fn with_written_to_tmp_file<F, R, C: AsRef<[u8]>>(content: C, op: F) -> Result<R, CliError>
where
    F: FnOnce(&Path) -> Result<R, CliError>,
{
    let temp_file = NamedTempFile::new()
        .map_err(|e| TemporaryFileError::with_debug("failed to create NamedTempFile", &e))?;
    std::fs::write(&temp_file.path(), content)
        .map_err(|e| TemporaryFileError::with_debug("failed to write content", &e))?;
    let result = op(&temp_file.path());
    // NamedTempFiles are deleted automatically when they go out of scope, but
    // drop it explicitly just to be safe.
    drop(temp_file);
    result
}

/// Similar to `with_written_to_tmp_file`, but creates an empty file.
pub fn with_tmp_file<F, R>(op: F) -> Result<R, CliError>
where
    F: FnOnce(&Path) -> Result<R, CliError>,
{
    let temp_file = NamedTempFile::new()
        .map_err(|e| TemporaryFileError::with_debug("failed to create NamedTempFile", &e))?;
    let result = op(&temp_file.path());
    // NamedTempFiles are deleted automatically when they go out of scope, but
    // drop it explicitly just to be safe.
    drop(temp_file);
    result
}

/// Perform an operation with 'content' written temporarily to 'path'.
///
/// Internally, for safety, this creates a temporary file using NamedTempFile,
/// and only a symlink to 'path'. This way, if the function fails unexpectedly,
/// the temporary file will still be deleted even if cleanup doesn't run (just
/// leaving a dangling symlink).
pub fn with_written_to_tmp_file_at_path<F, R, C: AsRef<[u8]>>(
    pr: &Printer,
    content: C,
    path: &Path,
    op: F,
) -> Result<R, CliError>
where
    F: FnOnce() -> Result<R, CliError>,
{
    pr.info(&format!("Writing to temporary file at {}.", path.display()));
    if path.exists() {
        return Err(TemporaryFileError::new(
            "a file at the desired path already exists",
        ));
    }
    let temp_file = NamedTempFile::new()
        .map_err(|e| TemporaryFileError::with_debug("failed to create NamedTempFile", &e))?;
    std::fs::write(&temp_file.path(), content)
        .map_err(|e| TemporaryFileError::with_debug("failed to write content", &e))?;
    ln_s(&temp_file.path(), path)
        .map_err(|e| TemporaryFileError::with_debug("failed to create symlink", &e))?;
    let result = op();
    rm(path).map_err(|e| TemporaryFileError::with_debug("failed to remove symlink", &e))?;
    drop(temp_file);
    result
}
