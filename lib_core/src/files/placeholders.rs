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
    let mut all_placeholders = Vec::new();

    // Replace all placeholders with their corresponding values.
    let result = placeholder_pattern.replace_all(&file_content, |caps: &regex::Captures| {
        let key = &caps[1]; // Extract the key inside {{ }}
        all_placeholders.push(key.to_string());
        placeholders
            .get(key)
            .cloned()
            .unwrap_or_else(|| caps[0].to_string())
    });

    let replaced_content = result.into_owned();

    // Check if there are any unreplaced placeholders.
    if error_if_unreplaced_placeholders_remain {
        let unreplaced: Vec<_> = all_placeholders
            .into_iter()
            .filter(|key| !placeholders.contains_key(key))
            .collect();

        if !unreplaced.is_empty() {
            return Err(UnreplacedPlaceholdersRemain::new(
                &src.display(),
                &unreplaced,
            ));
        }
    }

    // Write the updated content back to the file.
    let mut file = File::create(dst).map_err(|e| IOError::with_debug(&e))?;
    file.write_all(replaced_content.as_bytes())
        .map_err(|e| IOError::with_debug(&e))?;

    Ok(())
}
