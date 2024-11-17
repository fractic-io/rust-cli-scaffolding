use regex::Regex;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Display, PathBuf};

use crate::{define_cli_error, CliError, IOError};

define_cli_error!(
    UnreplacedPlaceholdersRemain,
    "Unexpected placeholders remain in file '{file}': {unreplaced:?}.",
    { file: &Display<'_>, unreplaced: &Vec<String> }
);

pub fn replace_all_placeholders_in_file(
    src: &PathBuf,
    dst: &PathBuf,
    placeholders: &HashMap<String, String>,
    error_if_unreplaced_placeholders_remain: bool,
) -> Result<(), CliError> {
    // Read the file contents.
    let mut file_content = String::new();
    File::open(src)
        .map_err(|e| IOError::with_debug(&e))?
        .read_to_string(&mut file_content)
        .map_err(|e| IOError::with_debug(&e))?;

    // Use a regex to find placeholders of the form {{Key}}.
    let placeholder_pattern =
        Regex::new(r"\{\{(\w+)\}\}").expect("hardcoded regex should be valid");

    // Replace all placeholders with their corresponding values.
    let mut unknown_keys = Vec::new();
    let result = placeholder_pattern.replace_all(&file_content, |caps: &regex::Captures| {
        let key = &caps[1]; // The content inside {{ }}.
        if let Some(value) = placeholders.get(key) {
            value.clone()
        } else {
            unknown_keys.push(key.to_string());
            caps[0].to_string() // The full '{{Key}}' string.
        }
    });

    let replaced_content = result.into_owned();

    if error_if_unreplaced_placeholders_remain && !unknown_keys.is_empty() {
        return Err(UnreplacedPlaceholdersRemain::new(
            &src.display(),
            &unknown_keys,
        ));
    }

    // Write the updated content back to the file.
    let mut file = File::create(dst).map_err(|e| IOError::with_debug(&e))?;
    file.write_all(replaced_content.as_bytes())
        .map_err(|e| IOError::with_debug(&e))?;

    Ok(())
}
