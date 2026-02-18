use std::{
    fs,
    io::{self, Read as _, Write as _},
    path::Path,
    process::ExitStatus,
};

use tokio::io::AsyncReadExt as _;

use crate::{define_cli_error, CliError, IOError};

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
define_cli_error!(
    TtyRequiredCommandMissing,
    "Required command not found: {command}.",
    { command: &str }
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
    /// Output and errors are captured. Neither is output to the terminal.
    Mute,
}

#[derive(Debug, Default)]
pub struct ExecuteOptions<'a> {
    pub dir: Option<&'a Path>,
    pub env: Option<Vec<(String, String)>>,
}

#[derive(Debug)]
pub struct Executor {
    background_processes: Vec<tokio::process::Child>,
}

impl Executor {
    pub fn new() -> Self {
        Executor {
            background_processes: Vec::new(),
        }
    }

    pub async fn has_command(&self, program: &str) -> bool {
        let program = program.trim();
        if program.is_empty() {
            return false;
        }

        #[cfg(windows)]
        {
            tokio::process::Command::new("cmd")
                .args(["/C", "where", "/Q"])
                .arg(program)
                .status()
                .await
                .map(|status| status.success())
                .unwrap_or(false)
        }

        #[cfg(not(windows))]
        {
            tokio::process::Command::new("sh")
                .args(["-c", "command -v -- \"$1\" >/dev/null 2>&1", "sh"])
                .arg(program)
                .status()
                .await
                .map(|status| status.success())
                .unwrap_or(false)
        }
    }

    pub async fn require_command(&self, program: &str) -> Result<(), CliError> {
        if self.has_command(program).await {
            Ok(())
        } else {
            Err(TtyRequiredCommandMissing::new(program))
        }
    }

    #[track_caller]
    pub async fn execute(
        &self,
        command: &str,
        args: &[&str],
        io_mode: IOMode,
    ) -> Result<String, CliError> {
        self.execute_with_options(command, args, io_mode, ExecuteOptions::default())
            .await
    }

    #[track_caller]
    pub async fn execute_with_options(
        &self,
        command: &str,
        args: &[&str],
        io_mode: IOMode,
        options: ExecuteOptions<'_>,
    ) -> Result<String, CliError> {
        let abs_dir = match options.dir {
            Some(p) => fs::canonicalize(p).map_err(|e| IOError::with_debug(&e))?,
            None => std::env::current_dir().map_err(|e| IOError::with_debug(&e))?,
        };
        let mut child = tokio::process::Command::new(command)
            .args(args)
            .current_dir(abs_dir)
            .envs(options.env.unwrap_or_default())
            .stdin(match io_mode {
                IOMode::Attach => std::process::Stdio::inherit(),
                IOMode::StreamOutput | IOMode::Silent | IOMode::Mute => std::process::Stdio::null(),
            })
            .stdout(match io_mode {
                IOMode::Attach => std::process::Stdio::inherit(),
                IOMode::StreamOutput | IOMode::Silent | IOMode::Mute => {
                    std::process::Stdio::piped()
                }
            })
            .stderr(match io_mode {
                IOMode::Attach => std::process::Stdio::inherit(),
                IOMode::StreamOutput | IOMode::Silent | IOMode::Mute => {
                    std::process::Stdio::piped()
                }
            })
            .spawn()
            .map_err(|e| TtyExecuteError::with_debug(&e))?;

        let mut collected_output = String::new();

        if io_mode != IOMode::Attach {
            if let Some(mut stdout) = child.stdout.take() {
                let mut buffer = [0; 1024];
                loop {
                    match stdout.read(&mut buffer).await {
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
                    match stderr.read(&mut buffer).await {
                        Ok(0) => break, // EOF reached
                        Ok(n) => {
                            let output = String::from_utf8_lossy(&buffer[..n]);
                            if io_mode != IOMode::Mute {
                                eprint!("{}", output);
                                io::stderr().flush().unwrap();
                            }
                            collected_output.push_str(&output);
                        }
                        Err(_) => break,
                    }
                }
            }
        }

        let status = child
            .wait()
            .await
            .map_err(|e| TtyExecuteError::with_debug(&e))?;
        if status.success() {
            Ok(collected_output.trim().to_string())
        } else {
            Err(TtyCommandFailed::new(status, &collected_output))
        }
    }

    #[track_caller]
    pub fn execute_sync(
        &self,
        command: &str,
        args: &[&str],
        io_mode: IOMode,
    ) -> Result<String, CliError> {
        self.execute_with_options_sync(command, args, io_mode, ExecuteOptions::default())
    }

    #[track_caller]
    pub fn execute_with_options_sync(
        &self,
        command: &str,
        args: &[&str],
        io_mode: IOMode,
        options: ExecuteOptions<'_>,
    ) -> Result<String, CliError> {
        let abs_dir = match options.dir {
            Some(p) => fs::canonicalize(p).map_err(|e| IOError::with_debug(&e))?,
            None => std::env::current_dir().map_err(|e| IOError::with_debug(&e))?,
        };
        let mut child = std::process::Command::new(command)
            .args(args)
            .current_dir(abs_dir)
            .envs(options.env.unwrap_or_default())
            .stdin(match io_mode {
                IOMode::Attach => std::process::Stdio::inherit(),
                IOMode::StreamOutput | IOMode::Silent | IOMode::Mute => std::process::Stdio::null(),
            })
            .stdout(match io_mode {
                IOMode::Attach => std::process::Stdio::inherit(),
                IOMode::StreamOutput | IOMode::Silent | IOMode::Mute => {
                    std::process::Stdio::piped()
                }
            })
            .stderr(match io_mode {
                IOMode::Attach => std::process::Stdio::inherit(),
                IOMode::StreamOutput | IOMode::Silent | IOMode::Mute => {
                    std::process::Stdio::piped()
                }
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
                            if io_mode != IOMode::Mute {
                                eprint!("{}", output);
                                io::stderr().flush().unwrap();
                            }
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

    pub async fn execute_background(
        &mut self,
        command: &str,
        args: &[&str],
        dir: Option<&str>,
    ) -> Result<(), CliError> {
        let abs_dir = fs::canonicalize(dir.unwrap_or(".")).map_err(|e| IOError::with_debug(&e))?;
        self.background_processes.push(
            tokio::process::Command::new(command)
                .args(args)
                .current_dir(abs_dir)
                .stdout(std::process::Stdio::null())
                .spawn()
                .map_err(|e| TtyExecuteError::with_debug(&e))?,
        );
        Ok(())
    }

    pub(crate) async fn resolve_background_processes(
        &mut self,
        printer: &Printer,
    ) -> Result<(), CliError> {
        let processes = self.background_processes.drain(..).collect::<Vec<_>>();
        for mut process in processes {
            let current_result = process.try_wait();
            match current_result {
                Ok(Some(_)) => {}
                Ok(None) => {
                    printer.info("Waiting for background process to finish...");
                }
                Err(e) => printer.error(&e.to_string()),
            }
            match process.wait().await {
                // Normally we should check 'status.success()', but it seems
                // that sometimes background processes end because of a signal.
                // For now, ignore this and only show an error if it returned
                // with non-zero exit code.
                Ok(status) if status.code().unwrap_or_default() == 0 => {}
                Ok(status) => return Err(TtyBackgroundCommandFailed::new(status)),
                Err(e) => return Err(TtyExecuteError::with_debug(&e)),
            }
        }
        Ok(())
    }

    pub(crate) async fn sudo_is_cached(&self) -> bool {
        self.execute("sudo", &["-n", "true"], IOMode::Mute)
            .await
            .is_ok()
    }

    pub(crate) async fn cache_sudo(&self) -> Result<(), CliError> {
        self.execute("sudo", &["echo", "-n"], IOMode::Attach)
            .await?;
        Ok(())
    }
}
