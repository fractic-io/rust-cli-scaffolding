use aws_sdk_route53::{
    types::{Change, ChangeAction, ChangeBatch, ResourceRecord, ResourceRecordSet, RrType},
    Client,
};
use lib_core::{define_cli_error, CliError};

use crate::shared_config::config_from_profile;

define_cli_error!(Route53Error, "Error running AWS Route53 command.");
define_cli_error!(InvalidIpAddress, "Invalid IP address: {ip_address}.", { ip_address: &str });

// Since Route53 is a global service, we can use any region.
const DEFAULT_REGION: &'static str = "us-east-1";

pub async fn set_route53_record(
    profile: &str,
    hosted_zone_id: &str,
    record_name: &str,
    ip_address: &str,
    ttl: i64,
) -> Result<(), CliError> {
    let client = Client::new(&config_from_profile(profile, DEFAULT_REGION).await);

    let record_set = ResourceRecordSet::builder()
        .name(record_name)
        .r#type(RrType::A)
        .ttl(ttl)
        .resource_records(
            ResourceRecord::builder()
                .value(ip_address)
                .build()
                .map_err(|e| InvalidIpAddress::with_debug(ip_address, &e))?,
        )
        .build()
        .map_err(|e| Route53Error::with_debug(&e))?;

    let change = Change::builder()
        .action(ChangeAction::Upsert)
        .resource_record_set(record_set)
        .build()
        .map_err(|e| Route53Error::with_debug(&e))?;

    let changes = ChangeBatch::builder()
        .changes(change)
        .build()
        .map_err(|e| Route53Error::with_debug(&e))?;

    client
        .change_resource_record_sets()
        .hosted_zone_id(hosted_zone_id)
        .change_batch(changes)
        .send()
        .await
        .map_err(|e| Route53Error::with_debug(&e))?;

    Ok(())
}
