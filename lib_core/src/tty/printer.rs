use colored::Colorize as _;

const LINE_LENGTH: usize = 80;

#[derive(Debug, Clone)]
pub struct Printer;

impl Printer {
    pub fn new() -> Self {
        Printer
    }

    fn wrap(message: &str) -> String {
        textwrap::wrap(message, LINE_LENGTH).join("\n")
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

    pub fn hr(&self) {
        println!("{}", "-".repeat(LINE_LENGTH).dimmed());
    }

    pub fn info(&self, message: &str) {
        println!("{}", Self::wrap(message).dimmed());
    }

    pub fn important(&self, message: &str) {
        println!("{}", Self::wrap(message).bold());
    }

    pub fn warn(&self, message: &str) {
        self.hr();
        println!("{}", Self::wrap(message).yellow());
        self.hr();
    }

    pub fn error(&self, message: &str) {
        eprintln!("{}", Self::wrap(message).red());
    }

    pub fn success(&self, message: &str) {
        println!("{}", Self::wrap(message).green());
    }
}
