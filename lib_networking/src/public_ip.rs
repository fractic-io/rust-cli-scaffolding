use lib_core::{define_cli_error, CliError};

define_cli_error!(
    FailedToDeterminePublicIp,
    "Failed to determine public IP address."
);
define_cli_error!(
    IpifyInvalidResponse,
    "The server used to determine the machine's public IP address returned an invalid response."
);

pub async fn get_public_ip() -> Result<String, CliError> {
    let response = reqwest::get("https://api64.ipify.org")
        .await
        .map_err(|e| FailedToDeterminePublicIp::with_debug(&e))?;
    let ip = response
        .text()
        .await
        .map_err(|e| IpifyInvalidResponse::with_debug(&e))?;
    Ok(ip)
}
