use aws_sdk_ecr::Client;
use base64::{prelude::BASE64_STANDARD, Engine};
use lib_core::{define_cli_error, CliError, InvalidUTF8};

use crate::shared_config::config_from_profile;

define_cli_error!(EcrError, "Error running AWS ECR command.");
define_cli_error!(EcrCredentialsError, "Error decoding ECR credentials: {details}.", { details: &str });

#[derive(Debug)]
pub struct EcrCredentials {
    pub username: String,
    pub password: String,
    pub proxy_endpoint: String,
}

pub async fn get_ecr_credentials(profile: &str, region: &str) -> Result<EcrCredentials, CliError> {
    let client = Client::new(&config_from_profile(profile, region).await);

    let auth_response = client
        .get_authorization_token()
        .send()
        .await
        .map_err(|e| EcrError::with_debug(&e))?;
    let auth_data = auth_response
        .authorization_data
        .as_ref()
        .and_then(|v| v.first())
        .ok_or_else(|| EcrCredentialsError::new("no authorization data found"))?;

    let token = BASE64_STANDARD
        .decode(
            auth_data
                .authorization_token
                .as_ref()
                .ok_or_else(|| EcrCredentialsError::new("no authorization token found"))?,
        )
        .map_err(|e| EcrCredentialsError::with_debug("failed to parse base64", &e))?;
    let token_str = String::from_utf8(token).map_err(|e| InvalidUTF8::with_debug(&e))?;
    let credentials: Vec<&str> = token_str.split(':').collect();

    Ok(EcrCredentials {
        username: credentials[0].to_string(),
        password: credentials[1].to_string(),
        proxy_endpoint: auth_data.proxy_endpoint.as_ref().unwrap().to_string(),
    })
}
