use std::fmt;

use crate::{define_cli_error, CliError};

define_cli_error!(SelectionError, "Selection failed.");
define_cli_error!(NoItemsError, "No {type_name} items to select from.", { type_name: &str });

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
        let items = self.into_iter().collect::<Vec<_>>();
        if items.is_empty() {
            return Err(NoItemsError::new(get_type_name::<T>()).into());
        }
        inquire::Select::new(get_type_name::<T>(), items)
            .with_vim_mode(true)
            .prompt()
            .map_err(|e| SelectionError::with_debug(&e))
    }

    fn multi_select(self) -> Result<Vec<T>, CliError> {
        let items = self.into_iter().collect::<Vec<_>>();
        if items.is_empty() {
            return Err(NoItemsError::new(get_type_name::<T>()).into());
        }
        inquire::MultiSelect::new(get_type_name::<T>(), items)
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
