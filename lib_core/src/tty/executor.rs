use std::{
    collections::HashMap,
    fs,
    io::{self, Read as _, Write as _},
    process::ExitStatus,
};

use regex::Regex;

use crate::{
    constants::{INCLUDE_IN_ENV, INCLUDE_IN_PATH, PATH},
    define_cli_error, CliError, IOError,
};

use super::Printer;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IOMode {
    /// Command attaches directly to the current terminal. Output can not be
    /// captured in this mode, so the result will be empty.
    Attach,
    /// Output is captured, and simultaneously streamed to the terminal.
    StreamOutput,
    /// Output is captured in the background. Only errors are streamed to the
    /// terminal.
    Silent,
}

#[derive(Debug)]
pub struct Executor {
    pub(crate) env: HashMap<String, String>,
    background_processes: Vec<std::process::Child>,
}

impl Executor {
    pub fn new(env_overrides: &HashMap<String, String>) -> Self {
        Executor {
            env: Self::build_env(env_overrides),
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

    pub fn execute(
        &self,
        program: &str,
        args: &[&str],
        dir: Option<&str>,
        io_mode: IOMode,
    ) -> Result<String, CliError> {
        let abs_dir = fs::canonicalize(dir.unwrap_or(".")).map_err(|e| IOError::with_debug(&e))?;
        let mut child = std::process::Command::new(program)
            .env_clear()
            .envs(&self.env)
            .args(args)
            .current_dir(abs_dir)
            .stdin(match io_mode {
                IOMode::Attach => std::process::Stdio::inherit(),
                IOMode::StreamOutput | IOMode::Silent => std::process::Stdio::null(),
            })
            .stdout(match io_mode {
                IOMode::Attach => std::process::Stdio::inherit(),
                IOMode::StreamOutput | IOMode::Silent => std::process::Stdio::piped(),
            })
            .stderr(match io_mode {
                IOMode::Attach => std::process::Stdio::inherit(),
                IOMode::StreamOutput | IOMode::Silent => std::process::Stdio::piped(),
            })
            .spawn()
            .map_err(|e| TtyExecuteError::with_debug(&e))?;

        let mut collected_output = String::new();

        if io_mode != IOMode::Attach {
            if let Some(mut stdout) = child.stdout.take() {
                let mut buffer = [0; 1024];
                loop {
                    match stdout.read(&mut buffer) {
                        Ok(0) => break, // EOF reached
                        Ok(n) => {
                            let output = String::from_utf8_lossy(&buffer[..n]);
                            if io_mode == IOMode::StreamOutput {
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
    ) -> Result<(), CliError> {
        let abs_dir = fs::canonicalize(dir.unwrap_or(".")).map_err(|e| IOError::with_debug(&e))?;
        self.background_processes.push(
            std::process::Command::new(program)
                .env_clear()
                .envs(&self.env)
                .args(args)
                .current_dir(abs_dir)
                .stdout(std::process::Stdio::null())
                .spawn()
                .map_err(|e| TtyExecuteError::with_debug(&e))?,
        );
        Ok(())
    }

    pub(crate) fn resolve_background_processes(
        &mut self,
        printer: &Printer,
    ) -> Result<(), CliError> {
        let processes = self.background_processes.drain(..).collect::<Vec<_>>();
        processes
            .into_iter()
            .map(|mut process| {
                let current_result = process.try_wait();
                match current_result {
                    Ok(Some(_)) => {}
                    Ok(None) => {
                        printer.info("Waiting for background process to finish...");
                    }
                    Err(e) => printer.error(&e.to_string()),
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
}
