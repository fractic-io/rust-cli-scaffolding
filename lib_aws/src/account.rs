use std::fmt::Display;

use lib_core::{define_cli_error, CliError};

define_cli_error!(AwsInvalidAccountId, "Invalid AWS account ID.");

pub fn require_aws_account_id(account_id: impl Display) -> Result<u64, CliError> {
    let account_id_str = account_id.to_string();
    if account_id_str.len() != 12 {
        return Err(AwsInvalidAccountId::with_debug(&account_id.to_string()));
    }
    account_id_str
        .parse()
        .map_err(|_| AwsInvalidAccountId::with_debug(&account_id.to_string()))
}
