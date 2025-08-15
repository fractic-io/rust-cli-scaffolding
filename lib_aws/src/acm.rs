use aws_sdk_acm::{types::CertificateStatus, Client};
use lib_core::{define_cli_error, CliError};

use crate::shared_config::config_from_profile;

define_cli_error!(AcmError, "Error running AWS ACM command.");

/// Returns the ARN of an issued ACM certificate that covers the provided domain, if any.
///
/// Matching rules:
/// - Exact match on the certificate's primary domain.
/// - Wildcard match on the primary domain (e.g., `*.example.com` matches `api.example.com`).
/// - Subject Alternative Names are also checked (exact and wildcard).
///
/// Preference is given to an exact match if multiple certificates could match. Otherwise, a
/// wildcard match is returned if found.
pub async fn get_acm_certificate_arn_for_domain(
    profile: &str,
    region: &str,
    domain: &str,
) -> Result<Option<String>, CliError> {
    let target = normalize_domain(domain);
    let client = Client::new(&config_from_profile(profile, region).await);

    let mut next_token: Option<String> = None;
    let mut wildcard_candidate: Option<String> = None;

    loop {
        let mut req = client
            .list_certificates()
            .certificate_statuses(CertificateStatus::Issued);
        if let Some(token) = &next_token {
            req = req.next_token(token);
        }

        let resp = req.send().await.map_err(|e| AcmError::with_debug(&e))?;

        for summary in resp.certificate_summary_list() {
            let Some(arn) = summary.certificate_arn() else {
                continue;
            };
            let primary = summary.domain_name().unwrap_or("");
            let primary_norm = normalize_domain(primary);

            // Prefer an exact match on the primary name.
            if primary_norm == target {
                return Ok(Some(arn.to_string()));
            }

            // Consider wildcard match on the primary name.
            if wildcard_candidate.is_none() && wildcard_matches(&primary_norm, &target) {
                wildcard_candidate = Some(arn.to_string());
            }

            // Inspect SANs if needed to find a better match (exact beats wildcard).
            if primary_norm != target {
                let details = client
                    .describe_certificate()
                    .certificate_arn(arn)
                    .send()
                    .await
                    .map_err(|e| AcmError::with_debug(&e))?;

                if let Some(cert) = details.certificate() {
                    // Check SANs for exact match first.
                    if cert
                        .subject_alternative_names()
                        .iter()
                        .any(|name| normalize_domain(name) == target)
                    {
                        return Ok(Some(arn.to_string()));
                    }

                    // Otherwise, consider wildcard match from SANs if we don't have one yet.
                    if wildcard_candidate.is_none() {
                        if cert.subject_alternative_names().iter().any(|name| {
                            let n = normalize_domain(name);
                            wildcard_matches(&n, &target)
                        }) {
                            wildcard_candidate = Some(arn.to_string());
                        }
                    }
                }
            }
        }

        next_token = resp.next_token.map(|s| s.to_string());
        if next_token.is_none() {
            break;
        }
    }

    Ok(wildcard_candidate)
}

fn normalize_domain(domain: &str) -> String {
    let d = domain.trim().trim_end_matches('.').to_lowercase();
    d
}

fn wildcard_matches(candidate: &str, target: &str) -> bool {
    if let Some(suffix) = candidate.strip_prefix("*.") {
        // '*.example.com' matches 'api.example.com' but not 'example.com'.
        target.ends_with(suffix) && target != suffix
    } else {
        false
    }
}
