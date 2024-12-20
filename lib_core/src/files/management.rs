use std::{
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use crate::{CliError, IOError};

#[track_caller]
pub fn cp<S, D>(src: S, dst: D) -> Result<(), CliError>
where
    S: AsRef<Path>,
    D: AsRef<Path>,
{
    std::fs::copy(src, dst).map_err(|e| IOError::with_debug(&e))?;
    Ok(())
}

#[track_caller]
pub fn mv<S, D>(src: S, dst: D) -> Result<(), CliError>
where
    S: AsRef<Path>,
    D: AsRef<Path>,
{
    // Use fs_extra to support moving between different filesystems (not
    // supported by std::fs::rename).
    if src.as_ref().is_dir() {
        // Unfortunately, this doesn't seem to work:
        // fs_extra::dir::move_dir(src, dst, &fs_extra::dir::CopyOptions::new().overwrite(true))
        //     .map_err(|e| IOError::with_debug(&e))?;
        //
        // So we can't support moving directories between different filesystems.
        std::fs::rename(src, dst).map_err(|e| IOError::with_debug(&e))?;
    } else {
        fs_extra::file::move_file(
            src,
            dst,
            &fs_extra::file::CopyOptions::new().overwrite(true),
        )
        .map_err(|e| IOError::with_debug(&e))?;
    }
    Ok(())
}

#[track_caller]
pub fn rm<P>(path: P) -> Result<(), CliError>
where
    P: AsRef<Path>,
{
    std::fs::remove_file(path).map_err(|e| IOError::with_debug(&e))?;
    Ok(())
}

#[track_caller]
pub fn rm_rf<P>(path: P) -> Result<(), CliError>
where
    P: AsRef<Path>,
{
    if !path.as_ref().exists() {
        return Ok(());
    }
    std::fs::remove_dir_all(path).map_err(|e| IOError::with_debug(&e))?;
    Ok(())
}

#[track_caller]
pub fn ln_s<S, D>(src: S, dst: D) -> Result<(), CliError>
where
    S: AsRef<Path>,
    D: AsRef<Path>,
{
    std::os::unix::fs::symlink(src, dst).map_err(|e| IOError::with_debug(&e))?;
    Ok(())
}

#[track_caller]
pub fn mkdir_p<P>(path: P) -> Result<(), CliError>
where
    P: AsRef<Path>,
{
    std::fs::create_dir_all(path).map_err(|e| IOError::with_debug(&e))?;
    Ok(())
}

#[track_caller]
pub fn rmdir<P>(path: P) -> Result<(), CliError>
where
    P: AsRef<Path>,
{
    std::fs::remove_dir(path).map_err(|e| IOError::with_debug(&e))?;
    Ok(())
}

/// 'mode' argument can be hardcoded conventiently as 0o644, 0o755, etc.
#[track_caller]
pub fn chmod<P>(path: P, mode: u32) -> Result<(), CliError>
where
    P: AsRef<Path>,
{
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .map_err(|e| IOError::with_debug(&e))?;
    Ok(())
}

#[track_caller]
pub fn ls<P>(path: P) -> Result<Vec<PathBuf>, CliError>
where
    P: AsRef<Path>,
{
    std::fs::read_dir(path)
        .map_err(|e| IOError::with_debug(&e))?
        .map(|entry| entry.map(|e| e.path()).map_err(|e| IOError::with_debug(&e)))
        .collect()
}

#[track_caller]
pub fn cp_r<S, D>(src: S, dst: D) -> Result<(), CliError>
where
    S: AsRef<Path>,
    D: AsRef<Path>,
{
    fs_extra::copy_items(
        &vec![src.as_ref()],
        dst.as_ref(),
        &fs_extra::dir::CopyOptions::new().overwrite(true),
    )
    .map_err(|e| IOError::with_debug(&e))?;
    Ok(())
}
