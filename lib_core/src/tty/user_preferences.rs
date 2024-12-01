use std::{
    collections::HashMap,
    fs,
    io::{self, Write as _},
    path::PathBuf,
};

use serde::{Deserialize, Serialize};
use serde_yaml::Value;

use crate::{mkdir_p, CliError};

#[derive(Debug)]
pub struct UserPreferences {
    preferences: PreferencesFileContent,
    preferences_path: PathBuf,
    script_name: &'static str,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct PreferencesFileContent {
    #[serde(default)]
    env: HashMap<String, String>,
    #[serde(default)]
    scripts: HashMap<String, HashMap<String, Value>>,
}

impl UserPreferences {
    pub fn new(preferences_path: PathBuf, script_name: &'static str) -> Self {
        let expanded_path = if preferences_path.to_string_lossy().starts_with('~') {
            let path_str = preferences_path.to_string_lossy().to_string();
            let expanded =
                path_str.replace('~', std::env::var("HOME").unwrap_or_default().as_str());
            PathBuf::from(expanded)
        } else {
            preferences_path
        };
        let preferences = Self::get_preferences(&expanded_path).unwrap_or_default();

        UserPreferences {
            preferences,
            preferences_path: expanded_path,
            script_name,
        }
    }

    fn get_preferences(path: &PathBuf) -> Option<PreferencesFileContent> {
        if path.exists() {
            fs::read_to_string(&path)
                .map(|content| serde_yaml::from_str(&content).unwrap_or_default())
                .ok()
        } else {
            None
        }
    }

    pub fn get_pref<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Option<T> {
        self.preferences
            .scripts
            .get(self.script_name)
            .and_then(|script_config| script_config.get(key))
            .and_then(|value| serde_yaml::from_value(value.clone()).ok())
    }

    pub fn set_pref<T: Serialize>(&mut self, key: &str, value: Option<T>) -> Result<(), CliError> {
        if let Some(value) = value {
            self.preferences
                .scripts
                .entry(self.script_name.to_owned())
                .or_default()
                .insert(
                    key.to_string(),
                    serde_yaml::to_value(value).expect("failed to serialize value"),
                );
        } else {
            if let Some(script_config) = self.preferences.scripts.get_mut(self.script_name) {
                script_config.remove(key);
                if script_config.is_empty() {
                    self.preferences.scripts.remove(self.script_name);
                }
            }
        }

        if let Ok(yaml) = serde_yaml::to_string(&self.preferences) {
            if let Some(parent) = self.preferences_path.parent() {
                mkdir_p(parent)?;
            }
            let _ = fs::write(&self.preferences_path, yaml);
        }

        Ok(())
    }

    pub fn ask_pref(&mut self, key: &str, prompt: &str) -> Result<Option<String>, CliError> {
        let default_value = self.get_pref::<String>(key);

        if let Some(ref default_value) = default_value {
            print!("{} [{}]: ", prompt, default_value);
        } else {
            print!("{}: ", prompt);
        }
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = match input.trim() {
            "" => None,
            input => Some(input.to_string()),
        };

        if input.is_none() && default_value.is_some() {
            Ok(default_value)
        } else {
            self.set_pref(key, input.clone())?;
            Ok(input)
        }
    }

    pub fn env_overrides(&self) -> &HashMap<String, String> {
        &self.preferences.env
    }
}
