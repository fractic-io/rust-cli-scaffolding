use std::path::Path;

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
