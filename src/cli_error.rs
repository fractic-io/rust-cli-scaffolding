use std::fmt;

use colored::Colorize;
use fractic_server_error::{ServerErrorBehaviour, ServerErrorTag};

const PRINT_WIDTH: usize = 80;

/// What a [`CliError`] reports. It decides how the error is printed and how
/// [`Tty::close`](crate::Tty::close) ends the run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliErrorKind {
    /// Something broke. Printed with its debug detail and the place that
    /// raised it.
    Failure,
    /// The command cannot do what was asked, and the message tells the user
    /// why: invalid input, nothing to select, a profile that is not set up.
    /// Printed without the place that raised it.
    User,
    /// The user backed out: Esc, Ctrl-C or "no" at a confirmation. Printed as
    /// one dim line; a caller that returns to a prompt of its own prints
    /// nothing.
    Cancelled,
}

pub trait CliErrorTrait: std::fmt::Debug + Send + Sync + 'static {
    fn kind(&self) -> CliErrorKind {
        CliErrorKind::Failure
    }
    fn tag(&self) -> Option<String>;
    fn context(&self) -> &String;
    fn message(&self) -> &String;
    fn debug(&self) -> Option<&String>;
    fn annotations(&self) -> &Vec<&'static str>;
    fn annotate(&mut self, annotation: &'static str);
}

pub type CliError = Box<dyn CliErrorTrait>;

impl fmt::Display for dyn CliErrorTrait {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let kind = self.kind();
        if kind == CliErrorKind::Cancelled {
            return write!(f, "{}", self.message().dimmed());
        }
        if let Some(tag) = self.tag() {
            write!(f, "{}", format!("{}; ", tag.to_uppercase()).bold().red())?;
        }
        write!(
            f,
            "{}",
            textwrap::fill(self.message(), PRINT_WIDTH).bold().red()
        )?;
        if let Some(debug) = self.debug() {
            write!(f, "\n\n{}", debug.red())?;
        }
        if kind == CliErrorKind::Failure {
            write!(f, "\n\n{}", self.context().dimmed())?;
        }
        for annotation in self.annotations() {
            write!(f, "\n\n{}", format!("NOTE: {}", annotation).bold().yellow())?;
        }
        Ok(())
    }
}

impl std::error::Error for dyn CliErrorTrait {}

// Annotations.
// --------------------------------------------------

pub trait AnnotatableResult {
    fn annotate(self, annotation: &'static str) -> Self;
}

impl<T> AnnotatableResult for Result<T, CliError> {
    fn annotate(self, annotation: &'static str) -> Self {
        self.map_err(|mut e| {
            e.annotate(annotation);
            e
        })
    }
}

// Definining custom CLI errors.
// --------------------------------------------------

pub struct CliErrorContext;

impl CliErrorContext {
    #[track_caller]
    pub fn capture() -> String {
        fractic_server_error::ServerErrorContext::Full.capture()
    }
}

/// Reports a CLI error being constructed to the process-wide error sink of
/// `fractic-server-error`: a failure as kind `cli`, a user error as `user`. A
/// cancel is the user backing out, not an error, and is not reported. Called
/// by `define_cli_error!`; not for direct use.
#[doc(hidden)]
pub fn __report_cli_error(
    kind: CliErrorKind,
    fingerprint: &str,
    template: &str,
    location: &std::panic::Location<'_>,
    message: &str,
    debug: Option<&str>,
    context: &str,
) {
    let kind = match kind {
        CliErrorKind::Failure => fractic_server_error::ErrorKind::Cli,
        CliErrorKind::User => fractic_server_error::ErrorKind::User,
        CliErrorKind::Cancelled => return,
    };
    fractic_server_error::__report(
        fingerprint,
        template,
        kind,
        location,
        message,
        debug,
        context,
    );
}

/// `define_cli_error!(Name, "Message.")` defines an error type of kind
/// [`CliErrorKind::Failure`] whose `new()` and `with_debug(&debug)` return a
/// [`CliError`]. Add `{ field: Type }` for arguments interpolated into the
/// message. [`define_cli_user_error!`] is the same for [`CliErrorKind::User`].
#[macro_export]
macro_rules! define_cli_error {
    ($name:ident, $msg:expr) => {
        $crate::define_cli_error!($name, $msg, {});
    };
    ($name:ident, $msg:expr, { $($arg:ident : $argtype:ty),* $(,)? }) => {
        $crate::define_cli_error!(
            @kind $crate::CliErrorKind::Failure, $name, $msg, { $($arg : $argtype),* }
        );
    };
    (@kind $kind:expr, $name:ident, $msg:expr, { $($arg:ident : $argtype:ty),* }) => {
        #[derive(Debug)]
        pub struct $name {
            context: String,
            message: String,
            debug: Option<String>,
            annotations: Vec<&'static str>,
        }

        impl $name {
            /// Stable grouping key: `module_path!()::TypeName`.
            #[allow(dead_code)]
            pub const FINGERPRINT: &str = concat!(module_path!(), "::", stringify!($name));

            #[allow(dead_code)]
            #[allow(
                clippy::new_ret_no_self,
                reason = "callers rely on `new` returning the boxed `CliError`"
            )]
            #[track_caller]
            pub fn new($($arg: $argtype),*) -> $crate::CliError {
                let location = ::std::panic::Location::caller();
                let error = $name {
                    context: $crate::CliErrorContext::capture(),
                    message: format!($msg, $($arg = $arg),*),
                    debug: None,
                    annotations: Vec::new(),
                };
                $crate::__report_cli_error(
                    $kind,
                    Self::FINGERPRINT,
                    $msg,
                    location,
                    &error.message,
                    None,
                    &error.context,
                );
                Box::new(error)
            }

            #[allow(dead_code)]
            #[track_caller]
            pub fn with_debug<D>(
                $($arg: $argtype,)*
                debug: &D,
            ) -> $crate::CliError where D: std::fmt::Debug {
                let location = ::std::panic::Location::caller();
                let error = $name {
                    context: $crate::CliErrorContext::capture(),
                    message: format!($msg, $($arg = $arg),*),
                    debug: Some(format!("{:#?}", debug)),
                    annotations: Vec::new(),
                };
                $crate::__report_cli_error(
                    $kind,
                    Self::FINGERPRINT,
                    $msg,
                    location,
                    &error.message,
                    error.debug.as_deref(),
                    &error.context,
                );
                Box::new(error)
            }
        }

        impl $crate::CliErrorTrait for $name {
            fn kind(&self) -> $crate::CliErrorKind {
                $kind
            }
            fn tag(&self) -> Option<String> {
                None
            }
            fn context(&self) -> &String {
                &self.context
            }
            fn message(&self) -> &String {
                &self.message
            }
            fn debug(&self) -> Option<&String> {
                self.debug.as_ref()
            }
            fn annotations(&self) -> &Vec<&'static str> {
                &self.annotations
            }
            fn annotate(&mut self, annotation: &'static str) {
                self.annotations.push(annotation);
            }
        }
    };
}

/// [`define_cli_error!`] for an error of kind [`CliErrorKind::User`].
#[macro_export]
macro_rules! define_cli_user_error {
    ($name:ident, $msg:expr) => {
        $crate::define_cli_user_error!($name, $msg, {});
    };
    ($name:ident, $msg:expr, { $($arg:ident : $argtype:ty),* $(,)? }) => {
        $crate::define_cli_error!(
            @kind $crate::CliErrorKind::User, $name, $msg, { $($arg : $argtype),* }
        );
    };
}

// Standard errors.
// --------------------------------------------------

define_cli_error!(CriticalError, "Unexpected: {details}.", { details: &str });
define_cli_error!(MultithreadingError, "Error executing child threads.");
define_cli_error!(IOError, "IO error.");
define_cli_error!(InvalidUTF8, "Could not parse bytes as UTF-8.");
// The one error of kind `Cancelled`: a caller tests the kind, never the type.
define_cli_error!(@kind CliErrorKind::Cancelled, UserCancelled, "User cancelled operation.", {});

// Conversion from ServerError.
// --------------------------------------------------

#[derive(Debug)]
struct FromServerError(fractic_server_error::ServerError, Vec<&'static str>);

impl CliErrorTrait for FromServerError {
    /// A message the API forwards to its client is written for the user.
    fn kind(&self) -> CliErrorKind {
        match self.0.behaviour() {
            ServerErrorBehaviour::ForwardToClient
            | ServerErrorBehaviour::LogWarningForwardToClient => CliErrorKind::User,
            ServerErrorBehaviour::LogErrorForwardToClient
            | ServerErrorBehaviour::LogWarningSendFixedMsgToClient(_)
            | ServerErrorBehaviour::LogErrorSendFixedMsgToClient(_)
            | ServerErrorBehaviour::ReturnInternalServerError
            | ServerErrorBehaviour::ReturnUnauthorized => CliErrorKind::Failure,
        }
    }
    fn tag(&self) -> Option<String> {
        match self.0.tag() {
            ServerErrorTag::None => None,
            _ => Some(format!("{:?}", self.0.tag())),
        }
    }
    fn context(&self) -> &String {
        self.0.context()
    }
    fn message(&self) -> &String {
        self.0.message()
    }
    fn debug(&self) -> Option<&String> {
        self.0.debug()
    }
    fn annotations(&self) -> &Vec<&'static str> {
        &self.1
    }
    fn annotate(&mut self, annotation: &'static str) {
        self.1.push(annotation);
    }
}

impl From<fractic_server_error::ServerError> for CliError {
    fn from(error: fractic_server_error::ServerError) -> CliError {
        Box::new(FromServerError(error, Vec::new()))
    }
}

// Tests.
// --------------------------------------------------

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, OnceLock};

    use fractic_server_error::{define_internal_error, define_user_error, install_error_sink};

    use super::*;

    /// `(fingerprint, kind)` of every error the process-wide sink saw.
    fn reported() -> &'static Mutex<Vec<(String, fractic_server_error::ErrorKind)>> {
        static SEEN: OnceLock<Mutex<Vec<(String, fractic_server_error::ErrorKind)>>> =
            OnceLock::new();
        SEEN.get_or_init(|| {
            install_error_sink(Box::new(|event| {
                if let Some(seen) = SEEN.get() {
                    if let Ok(mut seen) = seen.lock() {
                        seen.push((event.fingerprint.to_owned(), event.kind));
                    }
                }
            }));
            Mutex::new(Vec::new())
        })
    }

    define_cli_error!(Broke, "It broke: {details}.", { details: &str });
    define_cli_user_error!(NothingToSelect, "Nothing to select.");

    #[test]
    fn a_failure_prints_its_debug_detail_and_where_it_was_raised() {
        let error = Broke::with_debug("disk", &"cause");
        assert_eq!(error.kind(), CliErrorKind::Failure);
        let printed = error.to_string();
        assert!(printed.contains("It broke: disk."));
        assert!(printed.contains("cause"));
        assert!(printed.contains(file!()));
    }

    #[test]
    fn a_user_error_prints_no_source_location() {
        let mut error = NothingToSelect::with_debug(&"cause");
        error.annotate("Create one first.");
        assert_eq!(error.kind(), CliErrorKind::User);
        let printed = error.to_string();
        assert!(printed.contains("Nothing to select."));
        assert!(printed.contains("cause"));
        assert!(printed.contains("NOTE: Create one first."));
        assert!(!printed.contains(file!()));
    }

    #[test]
    fn a_cancel_prints_only_its_message() {
        let error = UserCancelled::new();
        assert_eq!(error.kind(), CliErrorKind::Cancelled);
        let printed = error.to_string();
        assert!(printed.contains("User cancelled operation."));
        assert!(!printed.contains(file!()));
    }

    #[test]
    fn the_sink_sees_failures_and_user_errors_but_no_cancel() {
        let seen = reported();
        let _failure = Broke::new("disk");
        let _user = NothingToSelect::new();
        let _cancel = UserCancelled::new();

        let seen = seen.lock().unwrap();
        let kind_of = |fingerprint: &str| {
            seen.iter()
                .find(|(seen, _)| seen == fingerprint)
                .map(|(_, kind)| *kind)
        };
        assert_eq!(
            kind_of(Broke::FINGERPRINT),
            Some(fractic_server_error::ErrorKind::Cli)
        );
        assert_eq!(
            kind_of(NothingToSelect::FINGERPRINT),
            Some(fractic_server_error::ErrorKind::User)
        );
        assert_eq!(kind_of(UserCancelled::FINGERPRINT), None);
    }

    #[test]
    fn a_server_error_is_a_user_error_when_its_message_is_for_the_client() {
        define_user_error!(NotFound, "No such story.");
        define_internal_error!(Unreachable, "Database unreachable.");

        let user: CliError = NotFound::new().into();
        assert_eq!(user.kind(), CliErrorKind::User);
        assert!(!user.to_string().contains(file!()));

        let failure: CliError = Unreachable::new().into();
        assert_eq!(failure.kind(), CliErrorKind::Failure);
        assert!(failure.to_string().contains(file!()));
    }
}
