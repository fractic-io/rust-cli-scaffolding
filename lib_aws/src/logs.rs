use aws_sdk_cloudwatchlogs::Client;
use lib_core::{define_cli_error, CliError};

use crate::shared_config::config_from_profile;

define_cli_error!(
    CloudWatchLogsError,
    "Error running AWS CloudWatch Logs command."
);
define_cli_error!(InvalidWildcardPattern, "Invalid wildcard pattern: {pattern}.", { pattern: &str });
define_cli_error!(InvalidRetentionValue, "Invalid retention value: {value}.", { value: u32 });

/// Sets the retention policy in days for all CloudWatch log groups whose names
/// match the supplied wildcard pattern (supports `*` and `?`).
///
/// - `profile`: AWS named profile to use
/// - `region`: AWS region (e.g., "us-east-1")
/// - `log_group_wildcard`: pattern like "app-prod-*" or "service-?-logs"
/// - `retention_in_days`: number of days for retention policy
pub async fn set_log_retention_by_wildcard(
    profile: &str,
    region: &str,
    log_group_wildcard: &str,
    retention_in_days: u32,
) -> Result<(), CliError> {
    let retention_in_days = retention_in_days
        .try_into()
        .map_err(|e| InvalidRetentionValue::with_debug(retention_in_days, &e))?;

    let config = config_from_profile(profile, region).await;
    let client = Client::new(&config);

    let matcher = WildcardMatcher::new(log_group_wildcard)
        .map_err(|e| InvalidWildcardPattern::with_debug(log_group_wildcard, &e))?;

    let mut next_token: Option<String> = None;
    loop {
        let mut req = client.describe_log_groups();
        if let Some(token) = &next_token {
            req = req.next_token(token);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| CloudWatchLogsError::with_debug(&e))?;

        for group in resp.log_groups().iter() {
            if let Some(name) = group.log_group_name() {
                if matcher.is_match(name) {
                    client
                        .put_retention_policy()
                        .log_group_name(name)
                        .retention_in_days(retention_in_days)
                        .send()
                        .await
                        .map_err(|e| CloudWatchLogsError::with_debug(&e))?;
                }
            }
        }

        next_token = resp.next_token().map(|s| s.to_string());
        if next_token.is_none() {
            break;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
//  Helpers.
// ---------------------------------------------------------------------------

struct WildcardMatcher {
    regex: regex::Regex,
}

impl WildcardMatcher {
    fn new(pattern: &str) -> Result<Self, regex::Error> {
        // Convert a shell-style wildcard to a Rust regex:
        // - Escape all regex meta characters first
        // - Replace escaped "\*" with ".*" and escaped "\?" with "."
        // - Anchor with ^ and $
        let mut regex_pattern = String::from("^");

        let mut chars = pattern.chars().peekable();
        while let Some(ch) = chars.next() {
            match ch {
                '*' => regex_pattern.push_str(".*"),
                '?' => regex_pattern.push('.'),
                // Escape regex metacharacters
                '.' | '+' | '(' | ')' | '|' | '{' | '}' | '[' | ']' | '^' | '$' | '\\' => {
                    regex_pattern.push('\\');
                    regex_pattern.push(ch);
                }
                other => regex_pattern.push(other),
            }
        }

        regex_pattern.push('$');
        let regex = regex::Regex::new(&regex_pattern)?;
        Ok(Self { regex })
    }

    fn is_match(&self, text: &str) -> bool {
        self.regex.is_match(text)
    }
}
