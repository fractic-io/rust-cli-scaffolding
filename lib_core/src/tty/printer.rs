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
        println!("{}\n", "↳ Complete".dimmed());
    }

    pub(crate) fn section_error(&self) {
        println!("{}\n", "↳ Error".bold().red());
    }

    pub fn subcommand_separator(&self, subcommand: &str) {
        println!(
            "{}\n{}\n",
            format!("Starting subcommand '{subcommand}'...").green(),
            "-".repeat(80).green()
        );
    }

    pub fn caution_box(&self, message: &str) {
        println!("{}", "-".repeat(80).dimmed());
        self.warn(&textwrap::wrap(message, 80).join("\n"));
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
