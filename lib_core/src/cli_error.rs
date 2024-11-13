use std::fmt;

use colored::Colorize;

pub trait CliErrorTrait: std::fmt::Debug + Send + Sync + 'static {
    fn details(&self) -> CliErrorDetails;
}

pub type CliError = Box<dyn CliErrorTrait>;

#[derive(Debug)]
pub enum CliErrorDetails<'a> {
    Custom {
        context: &'a String,
        message: &'a String,
        debug: Option<&'a String>,
    },
    FromServerError(&'a fractic_server_error::ServerError),
}

impl fmt::Display for dyn CliErrorTrait {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.details() {
            CliErrorDetails::Custom { message, .. } => {
                write!(f, "{}\n{:#?}", message.bold(), self.details())
            }
            CliErrorDetails::FromServerError(error) => write!(f, "{}", error),
        }
    }
}

impl std::error::Error for dyn CliErrorTrait {}

// Definining custom CLI errors.
// --------------------------------------------------

#[macro_export]
macro_rules! define_cli_error {
    ($name:ident, $msg:expr) => {
        define_cli_error!($name, $msg, {});
    };
    ($name:ident, $msg:expr, { $($arg:ident : $argtype:ty),* $(,)? }) => {
        #[derive(Debug)]
        pub struct $name {
            context: String,
            message: String,
            debug: Option<String>,
        }

        impl $name {
            #[allow(dead_code)]
            #[track_caller]
            pub fn new($($arg: $argtype),*) -> $crate::CliError {
                Box::new($name {
                    context: std::backtrace::Backtrace::force_capture().to_string(),
                    message: format!($msg, $($arg = $arg),*),
                    debug: None,
                })
            }

            #[allow(dead_code)]
            #[track_caller]
            pub fn with_debug<D>(
                $($arg: $argtype,)*
                debug: &D,
            ) -> $crate::CliError where D: std::fmt::Debug {
                Box::new($name {
                    context: std::backtrace::Backtrace::force_capture().to_string(),
                    message: format!($msg, $($arg = $arg),*),
                    debug: Some(format!("{:?}", debug)),
                })
            }
        }

        impl $crate::CliErrorTrait for $name {
            fn details(&self) -> $crate::CliErrorDetails {
                $crate::CliErrorDetails::Custom {
                    context: &self.context,
                    message: &self.message,
                    debug: self.debug.as_ref(),
                }
            }
        }
    };
}

// Standard errors.
// --------------------------------------------------

define_cli_error!(CriticalError, "Unexpected: {details}.", { details: &str });
define_cli_error!(MultithreadingError, "Error executing child threads.");
define_cli_error!(IOError, "IO error.");

// Conversion from ServerError.
// --------------------------------------------------

#[derive(Debug)]
struct FromServerError(fractic_server_error::ServerError);

impl CliErrorTrait for FromServerError {
    fn details(&self) -> CliErrorDetails {
        CliErrorDetails::FromServerError(&self.0)
    }
}

impl From<fractic_server_error::ServerError> for CliError {
    fn from(error: fractic_server_error::ServerError) -> CliError {
        Box::new(FromServerError(error))
    }
}
