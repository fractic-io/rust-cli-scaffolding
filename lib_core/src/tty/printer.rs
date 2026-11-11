use std::{
    future::Future,
    io::{StdoutLock, Write as _},
    pin::Pin,
};

use colored::{ColoredString, Colorize as _};
use notify_rust::Notification;
#[cfg(all(unix, not(target_os = "macos")))]
use notify_rust::Urgency;

use crate::{continue_after_enter, yes_no, CliError};

#[derive(Debug, Clone)]
pub struct Printer;

/// What a notification means to the reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotifyKind {
    /// Expected confirmation (connected, mounted, done).
    Routine,
    /// Something to go and look at (input required).
    Attention,
    /// A failure.
    Error,
}

#[derive(Debug)]
pub struct StatusBarPrinter {
    out: StdoutLock<'static>,
}

impl Printer {
    pub fn new() -> Self {
        Printer
    }

    pub(crate) fn section_open(&self, title: &str) {
        println!("{}", title.bold());
    }

    pub(crate) fn section_close(&self) {
        println!("{}\n", "↳ Complete".dimmed());
    }

    pub(crate) fn section_error(&self) {
        println!("{}\n", "↳ Error".bold().red());
    }

    pub fn subcommand_separator(&self, subcommand: &str) {
        println!(
            "{}\n{}\n",
            format!("Starting subcommand '{subcommand}'...").green(),
            "─".repeat(80).green()
        );
    }

    pub fn caution_box(&self, message: &str) {
        println!("{}", "-".repeat(80).dimmed());
        self.warn(&textwrap::wrap(message, 80).join("\n"));
        println!("{}", "-".repeat(80).dimmed());
    }

    pub fn with_status_bar<'a, T, F, Fut>(
        &'a mut self,
        f: F,
    ) -> Pin<Box<dyn Future<Output = T> + 'a>>
    where
        F: FnOnce(StatusBarPrinter) -> Fut + 'a,
        Fut: Future<Output = T> + 'a,
    {
        Box::pin(async move { f(StatusBarPrinter::new()).await })
    }

    pub fn info(&self, message: &str) {
        println!("{}", message.dimmed());
    }

    pub fn important(&self, message: &str) {
        println!("{}", message.bright_blue());
    }

    pub fn warn(&self, message: &str) {
        println!("{}", message.yellow());
    }

    pub fn error(&self, message: &str) {
        eprintln!("{}", message.red());
    }

    pub fn success(&self, message: &str) {
        println!("{}", message.green());
    }

    pub fn yes_no(&self, prompt: &str) -> Result<bool, CliError> {
        self.notify(NotifyKind::Attention, "Input Required", prompt);
        yes_no(prompt)
    }

    pub fn continue_after_enter(&self, message: Option<&str>) -> Result<(), CliError> {
        self.notify(
            NotifyKind::Attention,
            "Input Required",
            message.unwrap_or("Press Enter to continue..."),
        );
        continue_after_enter(message)
    }

    /// Best effort. Does not fail if notifications are not supported.
    ///
    /// Relayed when a `ctrl` binary is on `PATH`; otherwise shown directly,
    /// and not at all inside SSH.
    pub fn notify(&self, kind: NotifyKind, title: &str, message: &str) {
        if std::env::var_os(NOTIFY_RELAYED_ENV).is_none() && on_path(NOTIFY_RELAY) {
            relay_notification(kind, title, message);
        } else {
            show_notification(kind, title, message);
        }
    }
}

impl StatusBarPrinter {
    fn new() -> Self {
        StatusBarPrinter {
            out: std::io::stdout().lock(),
        }
    }

    fn write(&mut self, status: ColoredString) {
        write!(self.out, "\r\x1b[2K").unwrap();
        write!(self.out, "\r{}", status).unwrap();
        self.out.flush().unwrap();
    }

    pub fn info(&mut self, status: &str) {
        self.write(status.dimmed());
    }

    pub fn important(&mut self, status: &str) {
        self.write(status.bright_blue());
    }

    pub fn warn(&mut self, status: &str) {
        self.write(status.yellow());
    }

    pub fn error(&mut self, status: &str) {
        self.write(status.red());
    }

    fn close(&mut self) {
        write!(self.out, "\n").unwrap();
        self.out.flush().unwrap();
    }
}

impl Drop for StatusBarPrinter {
    fn drop(&mut self) {
        self.close();
    }
}

// Internal: Notifications.
// ----------------------------------------------------------------------------

const NOTIFY_RELAY: &str = "ctrl";

/// Set on the relay process, so that a relay which fails (and notifies
/// about its own failure through this crate) does not relay again.
const NOTIFY_RELAYED_ENV: &str = "FRACTIC_CLI_NOTIFY_RELAYED";

impl NotifyKind {
    /// The relay's `--kind` value.
    fn as_arg(self) -> &'static str {
        match self {
            NotifyKind::Routine => "routine",
            NotifyKind::Attention => "attention",
            NotifyKind::Error => "error",
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    fn urgency(self) -> Urgency {
        match self {
            NotifyKind::Routine => Urgency::Low,
            NotifyKind::Attention => Urgency::Normal,
            NotifyKind::Error => Urgency::Critical,
        }
    }
}

#[cfg_attr(not(all(unix, not(target_os = "macos"))), allow(unused_variables))]
fn show_notification(kind: NotifyKind, title: &str, message: &str) {
    if ["SSH_CONNECTION", "SSH_CLIENT", "SSH_TTY"]
        .iter()
        .any(|key| std::env::var_os(key).is_some())
    {
        return;
    }
    let mut notification = Notification::new();
    notification.summary(title).body(message);
    #[cfg(all(unix, not(target_os = "macos")))]
    notification.urgency(kind.urgency());
    let _ = notification.show();
}

/// Runs the relay in the background so a slow SSH hop never delays the
/// prompt that follows, and falls back to showing the notification
/// directly when it fails. Sync on purpose: `notify` is called outside the
/// runtime too, so this cannot go through `Executor`.
fn relay_notification(kind: NotifyKind, title: &str, message: &str) {
    // The name notify-rust would show for this process.
    let app_name = std::env::current_exe()
        .ok()
        .and_then(|exe| Some(exe.file_stem()?.to_string_lossy().into_owned()))
        .unwrap_or_default();
    let child = std::process::Command::new(NOTIFY_RELAY)
        .args(["notify", "send", title, message, "--app-name", &app_name])
        .args(["--kind", kind.as_arg()])
        .env(NOTIFY_RELAYED_ENV, "1")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    let (title, message) = (title.to_owned(), message.to_owned());
    std::thread::spawn(move || {
        let relayed = child
            .and_then(|mut c| c.wait())
            .is_ok_and(|status| status.success());
        if !relayed {
            show_notification(kind, &title, &message);
        }
    });
}

fn on_path(binary: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(binary).is_file()))
}
