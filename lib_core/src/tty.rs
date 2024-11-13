use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_yaml::{self, Value};
use std::collections::HashMap;
use std::fs;
use std::future::Future;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::pin::Pin;
use std::process::ExitStatus;

use crate::constants::{
    ANDROID_HOME, FLUTTER_HOME, INCLUDE_IN_ENV, INCLUDE_IN_PATH, JAVA_HOME, PATH,
};
use crate::printer::Printer;
use crate::{define_cli_error, CliError, IOError};

define_cli_error!(TtyExecuteError, "Failed to execute command.");
define_cli_error!(
    TtyCommandFailed,
    "[{exit_status}] Command failed.\n{output}",
    { exit_status: ExitStatus, output: &str }
);
define_cli_error!(
    TtyBackgroundCommandFailed,
    "[{exit_status}] Background command failed.",
    { exit_status: ExitStatus }
);
define_cli_error!(
    MissingDependency,
    "{name} is required. To set the {name} path, set the {path_var} environment variable in '{preferences_file}'.\n\nCurrent overrides:\n{current_overrides:#?}",
    { name: &str, path_var: &str, preferences_file: &std::path::Display<'_>, current_overrides: &HashMap<String, String> }
);

pub struct Tty {
    preferences: Preferences,
    preferences_path: PathBuf,
    script_name: &'static str,
    env: HashMap<String, String>,
    printer: Printer,
    background_processes: Vec<std::process::Child>,
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
        let env = Self::build_env(&preferences.env);

        Self {
            preferences,
            preferences_path: expanded_path,
            script_name,
            env,
            printer: Printer::new(),
            background_processes: Vec::new(),
        }
    }

    fn build_env(env_overrides: &HashMap<String, String>) -> HashMap<String, String> {
        let mut env = HashMap::new();
        for var_name in INCLUDE_IN_ENV {
            if let Some(value) = env_overrides
                .get(&var_name.to_string())
                .cloned()
                .or_else(|| std::env::var(var_name).ok())
            {
                env.insert(var_name.to_string(), value);
            }
        }
        env.insert(PATH.to_string(), Self::build_path_var(&env));
        env
    }

    fn build_path_var(env: &HashMap<String, String>) -> String {
        let var_regex =
            Regex::new(r"\$([A-Z_][A-Z0-9_]*)").expect("Hardcoded regex should be valid.");
        let paths = INCLUDE_IN_PATH.iter().filter_map(|path| {
            // Find all variables in the path
            let vars_in_path: Vec<String> = var_regex
                .captures_iter(path)
                .map(|caps| caps[1].to_string())
                .collect();
            // Check if all variables are defined
            let mut all_vars_defined = true;
            let mut replacements = HashMap::new();
            for var_name in vars_in_path {
                if let Some(value) = env.get(&var_name).cloned() {
                    replacements.insert(var_name, value);
                } else {
                    all_vars_defined = false;
                    break;
                }
            }
            if all_vars_defined {
                // Replace variables
                let replaced_path = var_regex.replace_all(path, |caps: &regex::Captures| {
                    let var_name = &caps[1];
                    replacements.get(var_name).unwrap().clone()
                });
                Some(replaced_path.to_string())
            } else {
                None
            }
        });
        paths.collect::<Vec<_>>().join(":")
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
                    Value::String(serde_yaml::to_string(&value).unwrap()),
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

    pub fn info(&self, message: &str) {
        self.printer.info(message);
    }

    pub fn debug(&self, message: &str) {
        self.printer.debug(message);
    }

    pub fn warn(&self, message: &str) {
        self.printer.warn(message);
    }

    pub fn error(&self, message: &str) {
        self.printer.error(message);
    }

    pub fn success(&self, message: &str) {
        self.printer.success(message);
    }

    pub fn in_named_section<'a, T, F, Fut>(
        &'a mut self,
        name: &str,
        f: F,
    ) -> Pin<Box<dyn Future<Output = Result<T, CliError>> + 'a>>
    where
        F: FnOnce(&'a mut Self) -> Fut + 'a,
        Fut: Future<Output = Result<T, CliError>> + 'a,
    {
        let printer = self.printer.clone();
        printer.section_open(name);
        Box::pin(async move {
            let result = f(self).await;
            match result {
                Ok(value) => {
                    printer.section_close();
                    Ok(value)
                }
                Err(error) => {
                    printer.section_error();
                    Err(error)
                }
            }
        })
    }

    pub fn execute(
        &self,
        program: &str,
        args: &[&str],
        dir: Option<&str>,
        stream_output: bool,
    ) -> Result<String, CliError> {
        let abs_dir = fs::canonicalize(dir.unwrap_or(".")).map_err(|e| IOError::with_debug(&e))?;
        let mut child = std::process::Command::new(program)
            .env_clear()
            .envs(&self.env)
            .args(args)
            .current_dir(abs_dir)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| TtyExecuteError::with_debug(&e))?;

        let mut collected_output = String::new();

        if let Some(mut stdout) = child.stdout.take() {
            let mut buffer = [0; 1024];
            loop {
                match stdout.read(&mut buffer) {
                    Ok(0) => break, // EOF reached
                    Ok(n) => {
                        let output = String::from_utf8_lossy(&buffer[..n]);
                        if stream_output {
                            print!("{}", output);
                            io::stdout().flush().unwrap();
                        }
                        collected_output.push_str(&output);
                    }
                    Err(_) => break,
                }
            }
        }

        if let Some(mut stderr) = child.stderr.take() {
            let mut buffer = [0; 1024];
            loop {
                match stderr.read(&mut buffer) {
                    Ok(0) => break, // EOF reached
                    Ok(n) => {
                        let output = String::from_utf8_lossy(&buffer[..n]);
                        eprint!("{}", output);
                        io::stderr().flush().unwrap();
                        collected_output.push_str(&output);
                    }
                    Err(_) => break,
                }
            }
        }

        let status = child.wait().map_err(|e| TtyExecuteError::with_debug(&e))?;
        if status.success() {
            Ok(collected_output.trim().to_string())
        } else {
            Err(TtyCommandFailed::new(status, &collected_output))
        }
    }

    pub fn execute_background(
        &mut self,
        program: &str,
        args: &[&str],
        dir: Option<&str>,
        stream_output: bool,
    ) -> Result<(), CliError> {
        let abs_dir = fs::canonicalize(dir.unwrap_or(".")).map_err(|e| IOError::with_debug(&e))?;
        self.background_processes.push(
            std::process::Command::new(program)
                .env_clear()
                .envs(&self.env)
                .args(args)
                .current_dir(abs_dir)
                .stdout(match stream_output {
                    true => std::process::Stdio::inherit(),
                    false => std::process::Stdio::null(),
                })
                .spawn()
                .map_err(|e| TtyExecuteError::with_debug(&e))?,
        );
        Ok(())
    }

    pub fn resolve_background_processes(&mut self) -> Result<(), CliError> {
        let processes = self.background_processes.drain(..).collect::<Vec<_>>();
        processes
            .into_iter()
            .map(|mut process| {
                let current_result = process.try_wait();
                match current_result {
                    Ok(Some(_)) => {}
                    Ok(None) => {
                        self.debug("Waiting for background process to finish...");
                    }
                    Err(e) => self.error(&e.to_string()),
                }
                match process.wait() {
                    // Normally we should check 'status.success()', but it seems
                    // that sometimes background processes end because of a
                    // signal. For now, ignore this and only show an error if it
                    // returned with non-zero exit code.
                    Ok(status) if status.code().unwrap_or_default() == 0 => Ok(()),
                    Ok(status) => Err(TtyBackgroundCommandFailed::new(status)),
                    Err(e) => Err(TtyExecuteError::with_debug(&e)),
                }
            })
            .collect::<Result<_, CliError>>()
            .map_err(|e| e)
    }

    pub fn require(&self, dependencies: &[Dependency]) -> Result<(), CliError> {
        for dependency in dependencies {
            match dependency {
                Dependency::Java => {
                    self.execute("java", &["--version"], None, false)
                        .map_err(|e| {
                            MissingDependency::with_debug(
                                "Java",
                                &JAVA_HOME,
                                &self.preferences_path.display(),
                                &self.env,
                                &e,
                            )
                        })?;
                }
                Dependency::AndroidSdk => {
                    self.execute("sdkmanager", &["--version"], None, false)
                        .map_err(|e| {
                            MissingDependency::with_debug(
                                "Android SDK",
                                &ANDROID_HOME,
                                &self.preferences_path.display(),
                                &self.env,
                                &e,
                            )
                        })?;
                }
                Dependency::Flutter => {
                    self.execute("flutter", &["--version"], None, false)
                        .map_err(|e| {
                            MissingDependency::with_debug(
                                "Flutter",
                                &FLUTTER_HOME,
                                &self.preferences_path.display(),
                                &self.env,
                                &e,
                            )
                        })?;
                }
                Dependency::Command(command) => {
                    self.execute(command, &["--help"], None, false)
                        .map_err(|e| {
                            MissingDependency::with_debug(
                                command,
                                command,
                                &self.preferences_path.display(),
                                &self.env,
                                &e,
                            )
                        })?;
                }
            }
        }
        Ok(())
    }
}
