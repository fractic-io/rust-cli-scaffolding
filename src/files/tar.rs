use std::{
    fs::File,
    path::{Path, PathBuf},
};

use crate::{define_cli_error, CliError};

define_cli_error!(TarBundleError, "Error building tar bundle: {details}.", { details: &str });

pub fn build_tar_bundle<P: AsRef<Path>, Q: AsRef<Path>>(
    src_dir: P,
    dst_dir: Q,
) -> Result<PathBuf, CliError> {
    let tar_path = dst_dir.as_ref().join("bundle.tar");
    let tar_file = File::create(&tar_path)
        .map_err(|e| TarBundleError::with_debug("failed to create output file", &e))?;
    let mut tar = tar::Builder::new(tar_file);

    tar.append_dir_all(".", src_dir)
        .map_err(|e| TarBundleError::with_debug("failed to append directory to tar", &e))?;
    tar.finish()
        .map_err(|e| TarBundleError::with_debug("failed to finalize tar file", &e))?;

    Ok(tar_path)
}
