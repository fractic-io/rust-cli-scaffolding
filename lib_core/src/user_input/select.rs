use std::fmt;

use crate::{define_cli_error, CliError};

define_cli_error!(SelectionError, "Selection failed.");

pub trait Selectable {
    type Item;

    fn select(self) -> Result<Self::Item, CliError>;
    fn multi_select(self) -> Result<Vec<Self::Item>, CliError>;
}

impl<T: fmt::Display, Iter> Selectable for Iter
where
    Iter: IntoIterator<Item = T>,
{
    type Item = T;

    fn select(self) -> Result<T, CliError> {
        inquire::Select::new(get_type_name::<T>(), self.into_iter().collect())
            .with_vim_mode(true)
            .prompt()
            .map_err(|e| SelectionError::with_debug(&e))
    }

    fn multi_select(self) -> Result<Vec<T>, CliError> {
        inquire::MultiSelect::new(get_type_name::<T>(), self.into_iter().collect())
            .with_vim_mode(true)
            .prompt()
            .map_err(|e| SelectionError::with_debug(&e))
    }
}

fn get_type_name<T>() -> &'static str {
    std::any::type_name::<T>()
        .split("::")
        .last()
        .unwrap_or_default()
}
