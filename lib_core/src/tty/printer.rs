use std::{
    future::Future,
    io::{StdoutLock, Write as _},
    pin::Pin,
};

use colored::{ColoredString, Colorize as _};

#[derive(Debug, Clone)]
pub struct Printer;

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
