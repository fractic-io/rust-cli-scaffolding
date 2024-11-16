use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use crate::constants::{ANDROID_HOME, FLUTTER_HOME, JAVA_HOME};
use crate::{define_cli_error, CliError};

use super::executor::IOMode;
use super::{Executor, Printer, UserPreferences};

define_cli_error!(
    MissingDependency,
    "{name} is required. To set the {name} path, set the {path_var} environment variable in the user preferences file.\n\nCurrent overrides:\n{current_overrides:#?}",
    { name: &str, path_var: &str, current_overrides: &HashMap<String, String> }
);

pub struct Tty {
    start_time: std::time::Instant,
    printer: Printer,
    user_preferences: UserPreferences,
    executor: Executor,
}

pub enum Dependency {
    Java,
    AndroidSdk,
    Flutter,
    CargoLambda,
    AwsSam,
    Command(String),
}

impl Tty {
    pub fn new(preferences_path: PathBuf, script_name: &'static str) -> Self {
        let printer = Printer::new();
        let user_preferences = UserPreferences::new(preferences_path, script_name);
        let executor = Executor::new(user_preferences.env_overrides());

        Self {
            start_time: std::time::Instant::now(),
            printer,
            user_preferences,
            executor,
        }
    }

    pub fn subcommand_separator(&self, subcommand: &str) {
        self.printer.subcommand_separator(subcommand);
    }

    pub fn in_init_section<'a, T, F, Fut>(
        &'a mut self,
        f: F,
    ) -> Pin<Box<dyn Future<Output = Result<T, CliError>> + 'a>>
    where
        F: FnOnce(&'a mut Printer, &'a mut UserPreferences) -> Fut + 'a,
        Fut: Future<Output = Result<T, CliError>> + 'a,
    {
        let local_printer = self.printer.clone();
        local_printer.section_open("Initializing...");
        Box::pin(async move {
            let result = f(&mut self.printer, &mut self.user_preferences).await;
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

    pub fn in_exec_section<'a, T, F, Fut>(
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
                        .execute("java", &["--version"], None, IOMode::Silent)
                        .map_err(|e| {
                            MissingDependency::with_debug(
                                "Java",
                                &JAVA_HOME,
                                &self.executor.env,
                                &e,
                            )
                        })?;
                }
                Dependency::AndroidSdk => {
                    self.executor
                        .execute("sdkmanager", &["--version"], None, IOMode::Silent)
                        .map_err(|e| {
                            MissingDependency::with_debug(
                                "Android SDK",
                                &ANDROID_HOME,
                                &self.executor.env,
                                &e,
                            )
                        })?;
                }
                Dependency::Flutter => {
                    self.executor
                        .execute("flutter", &["--version"], None, IOMode::Silent)
                        .map_err(|e| {
                            MissingDependency::with_debug(
                                "Flutter",
                                &FLUTTER_HOME,
                                &self.executor.env,
                                &e,
                            )
                        })?;
                }
                Dependency::CargoLambda => {
                    self.executor
                        .execute("cargo", &["lambda", "--version"], None, IOMode::Silent)
                        .map_err(|e| {
                            MissingDependency::with_debug(
                                "Cargo Lambda",
                                "PATH",
                                &self.executor.env,
                                &e,
                            )
                        })?;
                }
                Dependency::AwsSam => {
                    self.executor
                        .execute("sam", &["--version"], None, IOMode::Silent)
                        .map_err(|e| {
                            MissingDependency::with_debug(
                                "AWS CLI SAM",
                                "PATH",
                                &self.executor.env,
                                &e,
                            )
                        })?;
                }
                Dependency::Command(command) => {
                    self.executor
                        .execute(command, &["--help"], None, IOMode::Silent)
                        .map_err(|e| {
                            MissingDependency::with_debug(command, command, &self.executor.env, &e)
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
                eprintln!("{e}");
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
