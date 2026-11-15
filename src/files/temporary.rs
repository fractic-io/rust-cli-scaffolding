use std::path::Path;

use tempfile::NamedTempFile;

use crate::{define_cli_error, CliError, Printer};

use super::{ln_s, mv, rm};

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

/// Temporarily edit a file by applying `edit` to its String contents while running `op`.
///
/// The original file is backed up to a sibling file with a ".bak" suffix (e.g.,
/// "file.txt" -> "file.txt.bak"). After `op` completes (whether it succeeds or
/// fails), the original file is restored from the backup. If writing the edited
/// contents fails, the function attempts to restore the original immediately and
/// returns an error.
pub fn with_tmp_edits_to_file<F, R, E>(
    pr: &Printer,
    path: &Path,
    edit: E,
    op: F,
) -> Result<R, CliError>
where
    F: FnOnce() -> Result<R, CliError>,
    E: FnOnce(String) -> String,
{
    pr.info(&format!("Temporarily editing file at {}.", path.display()));

    if !path.exists() {
        return Err(TemporaryFileError::new("file does not exist"));
    }

    // Read original contents.
    let original = std::fs::read_to_string(path)
        .map_err(|e| TemporaryFileError::with_debug("failed to read original file", &e))?;

    // Compute edited contents.
    let edited = edit(original);

    // Compute backup path as "<filename>.bak" in the same directory.
    let file_name = path
        .file_name()
        .ok_or_else(|| TemporaryFileError::new("invalid path; no file name"))?;
    let backup_name = format!("{}.bak", file_name.to_string_lossy());
    let backup_path = path.with_file_name(backup_name);

    // Move original to backup.
    mv(path, &backup_path)
        .map_err(|e| TemporaryFileError::with_debug("failed to back up original file", &e))?;

    // Write edited contents; on failure, attempt immediate restore.
    if let Err(e) = std::fs::write(path, edited) {
        let _ = mv(&backup_path, path);
        return Err(TemporaryFileError::with_debug(
            "failed to write edited file",
            &e,
        ));
    }

    // Run operation while edited file is in place.
    let result = op();

    // Always attempt to restore original from backup.
    if let Err(e) = mv(&backup_path, path) {
        return Err(TemporaryFileError::with_debug(
            "failed to restore original file from backup",
            &e,
        ));
    }

    result
}
