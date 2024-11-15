use colored::Colorize as _;

#[derive(Debug, Clone)]
pub struct Printer;

impl Printer {
    pub fn new() -> Self {
        Printer
    }

    pub(crate) fn section_open(&self, title: &str) {
        println!("\n{}", title.bold());
    }

    pub(crate) fn section_close(&self) {
        println!("↳ {}", "Complete".bold());
    }

    pub(crate) fn section_error(&self) {
        println!("{}", "↳ Error".bold().red());
    }

    pub(crate) fn br(&self) {
        println!();
    }

    pub(crate) fn hr(&self) {
        println!("{}", "-".repeat(80).dimmed());
    }

    pub fn info(&self, message: &str) {
        println!("{}", message.dimmed());
    }

    pub fn important(&self, message: &str) {
        println!("{}", message.bold());
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
