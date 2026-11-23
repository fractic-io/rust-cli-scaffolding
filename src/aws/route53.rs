use crate::{define_cli_error, CliError};
use aws_sdk_route53::{
    types::{
        AliasTarget, Change, ChangeAction, ChangeBatch, ResourceRecord, ResourceRecordSet, RrType,
    },
    Client,
};

use super::shared_config::aws_config_from_profile;

define_cli_error!(Route53Error, "Error running AWS Route53 command.");
define_cli_error!(InvalidDnsValue, "Invalid DNS record value: {value}.", { value: &str });
define_cli_error!(
    Route53HostedZoneNotFound,
    "No public Route53 hosted zone is named '{domain}'.",
    { domain: &str }
);

/// The type of a record set, as the Route53 SDK names it (`A`, `Cname`, `Mx`,
/// `Txt`, …).
pub use aws_sdk_route53::types::RrType as Route53RecordType;

/// Since Route53 is a global service, we can use any region.
const DEFAULT_REGION: &str = "us-east-1";
/// CloudFront's alias hosted zone ID is global and constant.
const CLOUDFRONT_ALIAS_HOSTED_ZONE_ID: &str = "Z2FDTNDATAQYW2";

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
        &[ip_address],
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
        &[value],
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

/// The id (`Z…`, without the `/hostedzone/` prefix) of the public hosted zone
/// named exactly `domain`, as the other functions here take it.
pub async fn require_route53_hosted_zone_id(
    profile: &str,
    domain: &str,
) -> Result<String, CliError> {
    let client = Client::new(&aws_config_from_profile(profile, DEFAULT_REGION).await);
    let response = client
        .list_hosted_zones_by_name()
        .dns_name(domain)
        .send()
        .await
        .map_err(|e| Route53Error::with_debug(&e))?;
    let wanted = route53_name(domain);
    response
        .hosted_zones()
        .iter()
        .find(|zone| {
            route53_name(zone.name()) == wanted
                && !zone.config().is_some_and(|config| config.private_zone())
        })
        .map(|zone| zone.id().trim_start_matches("/hostedzone/").to_string())
        .ok_or_else(|| Route53HostedZoneNotFound::new(domain))
}

/// The values of the record set `record_name`/`record_type`, as Route53
/// stores them (a TXT value with its quotes, an MX value with its priority),
/// or `None` when the zone has no such set. An alias record set has no
/// values.
pub async fn get_route53_record_values(
    profile: &str,
    hosted_zone_id: &str,
    record_type: Route53RecordType,
    record_name: &str,
) -> Result<Option<Vec<String>>, CliError> {
    let record = record_get_helper(profile, hosted_zone_id, record_type, record_name).await?;
    Ok(record.map(|set| {
        set.resource_records()
            .iter()
            .map(|record| record.value().to_string())
            .collect()
    }))
}

/// Creates or replaces the record set `record_name`/`record_type` with
/// `values`, each in the form Route53 stores (see `route53_txt_value`).
pub async fn set_route53_record_values(
    profile: &str,
    hosted_zone_id: &str,
    record_type: Route53RecordType,
    record_name: &str,
    values: &[impl AsRef<str>],
    ttl: i64,
) -> Result<(), CliError> {
    record_set_helper(
        profile,
        hosted_zone_id,
        record_type,
        record_name,
        values,
        ttl,
    )
    .await
}

/// A domain name in the form Route53 returns it: lower case, with the trailing
/// dot. Names compare equal in this form however they were entered.
pub fn route53_name(name: &str) -> String {
    format!("{}.", name.trim().trim_end_matches('.').to_lowercase())
}

/// Whether two record sets of `record_type` hold the same values, in any
/// order. Values other than TXT compare as `route53_name` says, because a zone
/// keeps host names (MX and CNAME values) with or without the trailing dot, as
/// they were entered.
pub fn route53_values_match(
    record_type: &Route53RecordType,
    left: &[impl AsRef<str>],
    right: &[impl AsRef<str>],
) -> bool {
    let canonical = |values: Vec<&str>| {
        let mut values: Vec<String> = values
            .into_iter()
            .map(|value| match record_type {
                RrType::Txt => value.to_string(),
                _ => route53_name(value),
            })
            .collect();
        values.sort();
        values
    };
    canonical(left.iter().map(AsRef::as_ref).collect())
        == canonical(right.iter().map(AsRef::as_ref).collect())
}

/// A TXT value in the form Route53 stores it: quoted.
pub fn route53_txt_value(value: &str) -> String {
    format!("\"{value}\"")
}

// Helpers.
// ----------------------------------------------------------------------------

async fn record_set_helper(
    profile: &str,
    hosted_zone_id: &str,
    rtype: RrType,
    name: &str,
    values: &[impl AsRef<str>],
    ttl: i64,
) -> Result<(), CliError> {
    let client = Client::new(&aws_config_from_profile(profile, DEFAULT_REGION).await);

    let mut record_set = ResourceRecordSet::builder()
        .name(name)
        .r#type(rtype)
        .ttl(ttl);
    for value in values {
        let value = value.as_ref();
        record_set = record_set.resource_records(
            ResourceRecord::builder()
                .value(value)
                .build()
                .map_err(|e| InvalidDnsValue::with_debug(value, &e))?,
        );
    }
    let record_set = record_set
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
    let client = Client::new(&aws_config_from_profile(profile, DEFAULT_REGION).await);

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
    let client = Client::new(&aws_config_from_profile(profile, DEFAULT_REGION).await);

    let response = client
        .list_resource_record_sets()
        .hosted_zone_id(hosted_zone_id)
        .start_record_name(record_name)
        .start_record_type(rtype.clone())
        .max_items(1)
        .send()
        .await
        .map_err(|e| Route53Error::with_debug(&e))?;

    let wanted = route53_name(record_name);

    let found = response
        .resource_record_sets()
        .iter()
        .find(|set| route53_name(set.name()) == wanted && set.r#type() == &rtype)
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

// Tests.
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::{route53_name, route53_txt_value, route53_values_match, Route53RecordType};

    #[test]
    fn names_compare_without_case_or_trailing_dot() {
        assert_eq!(route53_name("Bounce.Example.com."), "bounce.example.com.");
        assert_eq!(route53_name("bounce.example.com"), "bounce.example.com.");
    }

    #[test]
    fn host_values_compare_without_the_trailing_dot() {
        let stored = ["10 inbound-smtp.eu-west-1.amazonaws.com."];
        let wanted = ["10 inbound-smtp.eu-west-1.amazonaws.com".to_string()];
        assert!(route53_values_match(
            &Route53RecordType::Mx,
            &stored,
            &wanted
        ));
        assert!(!route53_values_match(
            &Route53RecordType::Mx,
            &stored,
            &["10 inbound-smtp.ap-northeast-2.amazonaws.com"]
        ));
    }

    #[test]
    fn values_compare_in_any_order() {
        assert!(route53_values_match(
            &Route53RecordType::A,
            &["192.0.2.1", "192.0.2.2"],
            &["192.0.2.2", "192.0.2.1"]
        ));
    }

    #[test]
    fn txt_values_compare_exactly() {
        assert!(!route53_values_match(
            &Route53RecordType::Txt,
            &["\"v=spf1 -all\""],
            &["\"v=spf1 -all.\""]
        ));
    }

    #[test]
    fn txt_values_are_quoted() {
        assert_eq!(route53_txt_value("v=spf1 -all"), "\"v=spf1 -all\"");
    }
}
