use colored::*;

#[derive(Debug, Clone, Copy)]
pub struct Printer;

impl Printer {
    pub fn new() -> Self {
        Printer
    }

    pub fn section_open(&self, title: &str) {
        println!("{}", title.bold());
    }

    pub fn section_close(&self) {
        println!("↳ {}\n", "Complete".bold());
    }

    pub fn section_error(&self) {
        println!("{}\n", "↳ Error".bold().red());
    }

    pub fn debug(&self, message: &str) {
        println!("{}", message.dimmed());
    }

    pub fn info(&self, message: &str) {
        println!("{}", message.normal());
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
