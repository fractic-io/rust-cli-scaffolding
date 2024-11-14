use serde::{Deserialize, Serialize};
use serde_yaml::{self, Value};
use std::collections::HashMap;
use std::fs;
use std::future::Future;
use std::io::{self, Write};
use std::path::PathBuf;
use std::pin::Pin;

use crate::constants::{ANDROID_HOME, FLUTTER_HOME, JAVA_HOME};
use crate::{define_cli_error, CliError};

use super::{Executor, Printer};

define_cli_error!(
    MissingDependency,
    "{name} is required. To set the {name} path, set the {path_var} environment variable in '{preferences_file}'.\n\nCurrent overrides:\n{current_overrides:#?}",
    { name: &str, path_var: &str, preferences_file: &std::path::Display<'_>, current_overrides: &HashMap<String, String> }
);

pub struct Tty {
    start_time: std::time::Instant,
    preferences: Preferences,
    preferences_path: PathBuf,
    script_name: &'static str,
    printer: Printer,
    executor: Executor,
}

pub enum Dependency {
    Java,
    AndroidSdk,
    Flutter,
    Command(String),
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct Preferences {
    #[serde(default)]
    env: HashMap<String, String>,
    #[serde(default)]
    scripts: HashMap<String, HashMap<String, Value>>,
}

impl Tty {
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
        let printer = Printer::new();
        let executor = Executor::new(&preferences.env);

        Self {
            start_time: std::time::Instant::now(),
            preferences,
            preferences_path: expanded_path,
            script_name,
            printer,
            executor,
        }
    }

    fn get_preferences(path: &PathBuf) -> Option<Preferences> {
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

    pub fn set_pref<T: Serialize>(&mut self, key: &str, value: Option<T>) {
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
            let _ = fs::write(&self.preferences_path, yaml);
        }
    }

    pub fn ask_pref(&mut self, key: &str, prompt: &str) -> Option<String> {
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
            return default_value;
        } else {
            self.set_pref(key, input.clone());
            input
        }
    }

    pub fn hr(&self) {
        self.printer.hr();
    }

    pub fn in_named_section<'a, T, F, Fut>(
        &'a mut self,
        name: &str,
        f: F,
    ) -> Pin<Box<dyn Future<Output = Result<T, CliError>> + 'a>>
    where
        F: FnOnce(&'a mut Printer, &'a mut Executor) -> Fut + 'a,
        Fut: Future<Output = Result<T, CliError>> + 'a,
    {
        let local_printer = self.printer.clone();
        local_printer.section_open(name);
        Box::pin(async move {
            let result = f(&mut self.printer, &mut self.executor).await;
            match result {
                Ok(value) => {
                    local_printer.section_close();
                    Ok(value)
                }
                Err(error) => {
                    local_printer.section_error();
                    Err(error)
                }
            }
        })
    }

    pub fn require(&self, dependencies: &[Dependency]) -> Result<(), CliError> {
        for dependency in dependencies {
            match dependency {
                Dependency::Java => {
                    self.executor
                        .execute("java", &["--version"], None, false)
                        .map_err(|e| {
                            MissingDependency::with_debug(
                                "Java",
                                &JAVA_HOME,
                                &self.preferences_path.display(),
                                &self.executor.env,
                                &e,
                            )
                        })?;
                }
                Dependency::AndroidSdk => {
                    self.executor
                        .execute("sdkmanager", &["--version"], None, false)
                        .map_err(|e| {
                            MissingDependency::with_debug(
                                "Android SDK",
                                &ANDROID_HOME,
                                &self.preferences_path.display(),
                                &self.executor.env,
                                &e,
                            )
                        })?;
                }
                Dependency::Flutter => {
                    self.executor
                        .execute("flutter", &["--version"], None, false)
                        .map_err(|e| {
                            MissingDependency::with_debug(
                                "Flutter",
                                &FLUTTER_HOME,
                                &self.preferences_path.display(),
                                &self.executor.env,
                                &e,
                            )
                        })?;
                }
                Dependency::Command(command) => {
                    self.executor
                        .execute(command, &["--help"], None, false)
                        .map_err(|e| {
                            MissingDependency::with_debug(
                                command,
                                command,
                                &self.preferences_path.display(),
                                &self.executor.env,
                                &e,
                            )
                        })?;
                }
            }
        }
        Ok(())
    }

    pub fn close<T>(mut self, final_result: Result<T, CliError>) {
        let cleanup = self.executor.resolve_background_processes(&self.printer);
        match final_result.and(cleanup) {
            Ok(()) => {
                self.printer.success("SUCCESS");
                self.printer
                    .info(&format!("Elapsed: {}.", print_elapsed(self.start_time)));
            }
            Err(e) => {
                self.printer.error(&e.to_string());
                std::process::exit(1)
            }
        }
    }
}

fn print_elapsed(start: std::time::Instant) -> String {
    let elapsed = start.elapsed();
    let secs = elapsed.as_secs();
    let millis = elapsed.subsec_millis();
    if secs < 60 {
        format!("{}.{:03}s", secs, millis)
    } else {
        let mins = secs / 60;
        let secs = secs % 60;
        format!("{}m {}.{:03}s", mins, secs, millis)
    }
}
