use colored::Colorize as _;

#[derive(Debug, Clone)]
pub struct Printer;

impl Printer {
    pub fn new() -> Self {
        Printer
    }

    pub(crate) fn section_open(&self, title: &str) {
        println!("{}", title.bold());
    }

    pub(crate) fn section_close(&self) {
        println!("↳ {}\n", "Complete".bold());
    }

    pub(crate) fn section_error(&self) {
        println!("{}\n", "↳ Error".bold().red());
    }

    pub fn info(&self, message: &str) {
        println!("{}", message.dimmed());
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
