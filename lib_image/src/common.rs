use std::{fs, path::Path, sync::Once};

use lib_core::{CliError, CriticalError, IOError, Printer};
use magick_rust::{magick_wand_genesis, MagickWand, PixelWand};

use crate::ImageMagickError;

// Initialize MagickWand only once.
static START: Once = Once::new();

fn initialize_magick() {
    START.call_once(|| {
        magick_wand_genesis();
    });
}

pub fn with_image<P>(
    printer: &Printer,
    input: P,
    output: P,
    op: impl FnOnce(&mut MagickWand) -> Result<(), CliError>,
) -> Result<(), CliError>
where
    P: AsRef<Path>,
{
    initialize_magick();
    let mut wand = MagickWand::new();
    wand.read_image(
        input
            .as_ref()
            .to_str()
            .ok_or_else(|| CriticalError::new("invalid input path"))?,
    )
    .into_cli_res()?;
    op(&mut wand)?;
    if let Some(parent) = output.as_ref().parent() {
        fs::create_dir_all(parent).map_err(|e| IOError::with_debug(&e))?;
    }
    wand.write_image(
        output
            .as_ref()
            .to_str()
            .ok_or_else(|| CriticalError::new("invalid output path"))?,
    )
    .into_cli_res()?;
    printer.info(&format!("Image saved to {}.", output.as_ref().display()));
    Ok(())
}

pub fn write_to_tmp<C>(key: &str, bytes: C) -> Result<std::path::PathBuf, CliError>
where
    C: AsRef<[u8]>,
{
    let path = std::env::temp_dir().join(key);
    fs::write(&path, bytes).map_err(|e| IOError::with_debug(&e))?;
    Ok(path)
}

pub fn color(hex: &str) -> Result<PixelWand, CliError> {
    let mut c = PixelWand::new();
    c.set_color(hex).into_cli_res()?;
    Ok(c)
}

pub fn black() -> PixelWand {
    let mut c = PixelWand::new();
    c.set_color("#000000")
        .expect("Hard-coded color should be valid.");
    c
}

pub fn white() -> PixelWand {
    let mut c = PixelWand::new();
    c.set_color("#FFFFFF")
        .expect("Hard-coded color should be valid.");
    c
}

pub fn transparent() -> PixelWand {
    let mut c = PixelWand::new();
    c.set_color("#FFFFFF")
        .expect("Hard-coded color should be valid.");
    c
}

pub trait IntoCliError {
    fn into_cli_err(self) -> CliError;
}

impl IntoCliError for magick_rust::MagickError {
    fn into_cli_err(self) -> CliError {
        ImageMagickError::with_debug(&self)
    }
}

pub trait IntoCliResult<T> {
    fn into_cli_res(self) -> Result<T, CliError>;
}

impl<T> IntoCliResult<T> for Result<T, magick_rust::MagickError> {
    fn into_cli_res(self) -> Result<T, CliError> {
        self.map_err(|e| e.into_cli_err())
    }
}
