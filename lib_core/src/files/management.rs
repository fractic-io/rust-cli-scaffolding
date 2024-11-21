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
    std::fs::rename(src, dst).map_err(|e| IOError::with_debug(&e))?;
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
