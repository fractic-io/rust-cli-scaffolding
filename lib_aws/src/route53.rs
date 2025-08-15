use aws_sdk_route53::{
    types::{
        AliasTarget, Change, ChangeAction, ChangeBatch, ResourceRecord, ResourceRecordSet, RrType,
    },
    Client,
};
use lib_core::{define_cli_error, CliError};

use crate::shared_config::config_from_profile;

define_cli_error!(Route53Error, "Error running AWS Route53 command.");
define_cli_error!(InvalidDnsValue, "Invalid IP address: {ip_address}.", { ip_address: &str });

/// Since Route53 is a global service, we can use any region.
const DEFAULT_REGION: &'static str = "us-east-1";
/// CloudFront's alias hosted zone ID is global and constant.
const CLOUDFRONT_ALIAS_HOSTED_ZONE_ID: &'static str = "Z2FDTNDATAQYW2";

pub enum AliasTargetType<'a> {
    /// Automatically uses the global CloudFront hosted zone ID.
    CloudFront,
    /// For other targets (ALB/NLB, API Gateway, etc.) where the hosted zone ID
    /// must be provided.
    Explicit { hosted_zone_id: &'a str },
}

pub async fn set_route53_a_record(
    profile: &str,
    hosted_zone_id: &str,
    record_name: &str,
    ip_address: &str,
    ttl: i64,
) -> Result<(), CliError> {
    record_set_helper(
        profile,
        hosted_zone_id,
        RrType::A,
        record_name,
        ip_address,
        ttl,
    )
    .await
}

pub async fn set_route53_cname_record(
    profile: &str,
    hosted_zone_id: &str,
    record_name: &str,
    value: &str,
    ttl: i64,
) -> Result<(), CliError> {
    record_set_helper(
        profile,
        hosted_zone_id,
        RrType::Cname,
        record_name,
        value,
        ttl,
    )
    .await
}

pub async fn set_route53_a_record_alias<'a>(
    profile: &str,
    hosted_zone_id: &str,
    record_name: &str,
    target_type: AliasTargetType<'a>,
    dns_name: &str,
    evaluate_target_health: bool,
) -> Result<(), CliError> {
    alias_record_set_helper(
        profile,
        hosted_zone_id,
        RrType::A,
        record_name,
        target_type,
        dns_name,
        evaluate_target_health,
    )
    .await
}

pub async fn set_route53_cname_record_alias<'a>(
    profile: &str,
    hosted_zone_id: &str,
    record_name: &str,
    target_type: AliasTargetType<'a>,
    dns_name: &str,
    evaluate_target_health: bool,
) -> Result<(), CliError> {
    alias_record_set_helper(
        profile,
        hosted_zone_id,
        RrType::Cname,
        record_name,
        target_type,
        dns_name,
        evaluate_target_health,
    )
    .await
}

pub async fn get_route53_a_record(
    profile: &str,
    hosted_zone_id: &str,
    record_name: &str,
) -> Result<Option<String>, CliError> {
    let record = record_get_helper(profile, hosted_zone_id, RrType::A, record_name).await?;
    Ok(record.and_then(record_value_string))
}

pub async fn get_route53_cname_record(
    profile: &str,
    hosted_zone_id: &str,
    record_name: &str,
) -> Result<Option<String>, CliError> {
    let record = record_get_helper(profile, hosted_zone_id, RrType::Cname, record_name).await?;
    Ok(record.and_then(record_value_string))
}

// ---------------------------------------------------------------------------
//  Helpers.
// ---------------------------------------------------------------------------

async fn record_set_helper(
    profile: &str,
    hosted_zone_id: &str,
    rtype: RrType,
    name: &str,
    value: &str,
    ttl: i64,
) -> Result<(), CliError> {
    let client = Client::new(&config_from_profile(profile, DEFAULT_REGION).await);

    let record_set = ResourceRecordSet::builder()
        .name(name)
        .r#type(rtype)
        .ttl(ttl)
        .resource_records(
            ResourceRecord::builder()
                .value(value)
                .build()
                .map_err(|e| InvalidDnsValue::with_debug(value, &e))?,
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

async fn alias_record_set_helper<'a>(
    profile: &str,
    hosted_zone_id: &str,
    rtype: RrType,
    name: &str,
    target_type: AliasTargetType<'a>,
    dns_name: &str,
    evaluate_target_health: bool,
) -> Result<(), CliError> {
    let client = Client::new(&config_from_profile(profile, DEFAULT_REGION).await);

    let hosted_zone_id_for_target = match target_type {
        AliasTargetType::CloudFront => CLOUDFRONT_ALIAS_HOSTED_ZONE_ID,
        AliasTargetType::Explicit { hosted_zone_id } => hosted_zone_id,
    };

    let alias_target = AliasTarget::builder()
        .hosted_zone_id(hosted_zone_id_for_target)
        .dns_name(dns_name)
        .evaluate_target_health(evaluate_target_health)
        .build()
        .map_err(|e| Route53Error::with_debug(&e))?;

    let record_set = ResourceRecordSet::builder()
        .name(name)
        .r#type(rtype)
        .alias_target(alias_target)
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

async fn record_get_helper(
    profile: &str,
    hosted_zone_id: &str,
    rtype: RrType,
    record_name: &str,
) -> Result<Option<ResourceRecordSet>, CliError> {
    let client = Client::new(&config_from_profile(profile, DEFAULT_REGION).await);

    let response = client
        .list_resource_record_sets()
        .hosted_zone_id(hosted_zone_id)
        .start_record_name(record_name)
        .start_record_type(rtype.clone())
        .max_items(1)
        .send()
        .await
        .map_err(|e| Route53Error::with_debug(&e))?;

    let desired = normalize_record_name(record_name);

    let found = response
        .resource_record_sets()
        .iter()
        .find(|set| set.name() == desired && set.r#type() == &rtype)
        .cloned();

    Ok(found)
}

// Returns the most useful display string for a record set:
// - If alias, returns the alias target DNS name.
// - Otherwise, returns the first resource record value (if any).
fn record_value_string(set: ResourceRecordSet) -> Option<String> {
    if let Some(alias) = set.alias_target() {
        return Some(alias.dns_name().to_string());
    }
    set.resource_records()
        .first()
        .map(|rr| rr.value().to_string())
}

// Route 53 returns record names as absolute FQDNs (with a trailing dot). Users
// would expect pass names without the trailing dot, so normalize the input to
// ensure comparisons (and lookups) succeed regardless of user input format.
fn normalize_record_name(name: &str) -> String {
    if name.ends_with('.') {
        name.to_string()
    } else {
        format!("{}.", name)
    }
}
