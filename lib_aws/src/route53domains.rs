use aws_sdk_route53domains::{
    types::{builders::NameserverBuilder, Nameserver},
    Client,
};
use lib_core::{define_cli_error, CliError};

use crate::shared_config::config_from_profile;

define_cli_error!(
    Route53DomainError,
    "Error running AWS Route53 Domains command."
);
define_cli_error!(
    Route53DomainInvalidNameserver,
    "Invalid nameserver: {nameserver}.",
    { nameserver: &str }
);

// Since Route53 is a global service, we can use any region.
const DEFAULT_REGION: &'static str = "us-east-1";

pub async fn set_route53_domain_nameservers(
    profile: &str,
    domain: &str,
    nameservers: Vec<&str>,
) -> Result<(), CliError> {
    let client = Client::new(&config_from_profile(profile, DEFAULT_REGION).await);

    client
        .update_domain_nameservers()
        .domain_name(domain)
        .set_nameservers(Some(
            nameservers
                .into_iter()
                .map(|ns| {
                    NameserverBuilder::default()
                        .name(ns)
                        .build()
                        .map_err(|e| Route53DomainInvalidNameserver::with_debug(&ns, &e))
                })
                .collect::<Result<Vec<Nameserver>, CliError>>()?,
        ))
        .send()
        .await
        .map_err(|e| Route53DomainError::with_debug(&e))?;

    Ok(())
}
