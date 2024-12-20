use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use tokio::{select, signal};

use crate::{CliError, CtrlC};

use super::{Executor, Printer, UserPreferences};

pub struct Tty {
    start_time: std::time::Instant,
    printer: Printer,
    user_preferences: UserPreferences,
    executor: Executor,
}

impl Tty {
    pub fn new(preferences_path: PathBuf, script_name: &'static str) -> Result<Self, CliError> {
        let printer = Printer::new();
        let user_preferences = UserPreferences::new(preferences_path, script_name)?;
        let executor = Executor::new();

        Ok(Self {
            start_time: std::time::Instant::now(),
            printer,
            user_preferences,
            executor,
        })
    }

    pub fn subcommand_separator(&self, subcommand: &str) {
        self.printer.subcommand_separator(subcommand);
    }

    pub fn cache_sudo(&self) -> Result<(), CliError> {
        if !self.executor.sudo_is_cached() {
            self.printer.caution_box("This script requires sudo. Enter your password to cache credentials for the duration of the script.");
            self.executor.cache_sudo()?;
            self.printer.info("Credentials cached.\n");
        }
        Ok(())
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
            let result = select! {
                result = f(&mut self.printer, &mut self.executor) => result,
                _ = signal::ctrl_c() => {
                    Err(CtrlC::new())
                }
            };
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
