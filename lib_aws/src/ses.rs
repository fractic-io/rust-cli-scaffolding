use aws_sdk_ses::{types::VerificationStatus, Client};
use lib_core::{define_cli_error, CliError};

use crate::shared_config::config_from_profile;

define_cli_error!(
    SesIdentityValidationError,
    "Was unable to validate SES identity '{identity}' in region '{region}'.",
    { identity: &str, region: &str }
);
define_cli_error!(
    SesIdentityNotVerified,
    "SES identity '{identity}' is not verified in region '{region}'.",
    { identity: &str, region: &str }
);

pub async fn require_ses_identity(
    region: &str,
    profile: &str,
    identity: &str,
) -> Result<(), CliError> {
    let client = Client::new(&config_from_profile(region, profile).await);

    let result = client
        .get_identity_verification_attributes()
        .identities(identity)
        .send()
        .await
        .map_err(|e| SesIdentityValidationError::with_debug(identity, region, &e))?;

    result
        .verification_attributes
        .get(identity)
        .and_then(|attr| {
            if attr.verification_status == VerificationStatus::Success {
                Some(())
            } else {
                None
            }
        })
        .ok_or_else(|| SesIdentityNotVerified::new(identity, region))?;

    Ok(())
}
