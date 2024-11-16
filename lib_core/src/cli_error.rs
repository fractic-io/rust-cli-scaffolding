use std::fmt;

use colored::Colorize;
use fractic_server_error::ServerErrorTag;

const PRINT_WIDTH: usize = 80;

pub trait CliErrorTrait: std::fmt::Debug + Send + Sync + 'static {
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
        if let Some(tag) = self.tag() {
            write!(f, "{}", tag.bold().red())?;
        }
        write!(
            f,
            "{}",
            textwrap::fill(self.message(), PRINT_WIDTH).bold().red()
        )?;
        if let Some(debug) = self.debug() {
            write!(f, "\n\n{}", debug.red())?;
        }
        write!(f, "\n\n{}", self.context().dimmed())?;
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
            annotations: Vec<&'static str>,
        }

        impl $name {
            #[allow(dead_code)]
            #[track_caller]
            pub fn new($($arg: $argtype),*) -> $crate::CliError {
                Box::new($name {
                    context: std::backtrace::Backtrace::force_capture().to_string(),
                    message: format!($msg, $($arg = $arg),*),
                    debug: None,
                    annotations: Vec::new(),
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
                    debug: Some(format!("{:#?}", debug)),
                    annotations: Vec::new(),
                })
            }
        }

        impl $crate::CliErrorTrait for $name {
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

// Standard errors.
// --------------------------------------------------

define_cli_error!(CriticalError, "Unexpected: {details}.", { details: &str });
define_cli_error!(MultithreadingError, "Error executing child threads.");
define_cli_error!(IOError, "IO error.");

// Conversion from ServerError.
// --------------------------------------------------

#[derive(Debug)]
struct FromServerError(fractic_server_error::ServerError, Vec<&'static str>);

impl CliErrorTrait for FromServerError {
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
