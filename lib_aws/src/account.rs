use lib_core::{define_cli_error, CliError};

define_cli_error!(AwsInvalidAccountId, "Invalid AWS account ID.");

pub fn require_aws_account_id(account_id: &str) -> Result<(), CliError> {
    if account_id.len() != 12 {
        return Err(AwsInvalidAccountId::with_debug(&account_id.to_string()));
    }
    let _parsed_as_u64 = account_id
        .parse::<u64>()
        .map_err(|_| AwsInvalidAccountId::with_debug(&account_id.to_string()))?;
    Ok(())
}
