use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use aws_sdk_cloudformation::{
    client::Waiters,
    types::{Capability, Parameter},
    Client,
};
use chrono::SecondsFormat;
use lib_core::{define_cli_error, CliError, Printer};

use crate::shared_config::config_from_profile;

define_cli_error!(CloudFormationError, "Error running CloudFormation command.");
define_cli_error!(
    CloudFormationStackNotFound,
    "The CloudFormation stack '{stack_name}' does not exist in region '{region}'.",
    { stack_name: &str, region: &str }
);
define_cli_error!(
    CloudFormationOutputMissing,
    "The CloudFormation stack '{stack_name}' does not have required outputs {missing_output:?}.",
    { stack_name: &str, missing_output: HashSet<String> }
);
define_cli_error!(
    CloudFormationDeploymentFailed,
    "Failed to deploy CloudFormation stack '{stack_name}'.",
    { stack_name: &str }
);

const DEPLOY_WAIT_TIMEOUT: Duration = Duration::from_secs(30 * 60); // 30 minutes

pub enum StackDeploymentMethod {
    Changeset,
    Direct,
}

pub async fn stack_exists(profile: &str, region: &str, stack_name: &str) -> Result<bool, CliError> {
    let client = Client::new(&config_from_profile(profile, region).await);

    let response = client
        .describe_stacks()
        .stack_name(stack_name)
        .send()
        .await
        .map_err(|e| CloudFormationError::with_debug(&e))?;

    Ok(!response.stacks.unwrap_or_default().is_empty())
}

pub async fn require_stack_outputs(
    profile: &str,
    region: &str,
    stack_name: &str,
    output_keys: HashSet<String>,
) -> Result<HashMap<String, String>, CliError> {
    let client = Client::new(&config_from_profile(profile, region).await);

    let response = client
        .describe_stacks()
        .stack_name(stack_name)
        .send()
        .await
        .map_err(|e| CloudFormationError::with_debug(&e))?;

    let stack = response
        .stacks
        .unwrap_or_default()
        .into_iter()
        .next()
        .ok_or_else(|| CloudFormationStackNotFound::new(stack_name, region))?;

    let outputs = stack.outputs.unwrap_or_default();
    let outputs_map: std::collections::HashMap<String, String> = outputs
        .into_iter()
        .filter_map(|output| Some((output.output_key?, output.output_value?)))
        .collect();

    let result: HashMap<String, String> = outputs_map
        .into_iter()
        .filter(|(key, _)| output_keys.contains(key))
        .collect();

    if result.len() != output_keys.len() {
        let missing_outputs: HashSet<String> = output_keys
            .difference(&result.keys().cloned().collect())
            .cloned()
            .collect();
        return Err(CloudFormationOutputMissing::new(
            stack_name,
            missing_outputs,
        ));
    }

    Ok(result)
}

pub async fn require_stack_output(
    profile: &str,
    region: &str,
    stack_name: &str,
    output_key: &str,
) -> Result<String, CliError> {
    Ok(require_stack_outputs(
        profile,
        region,
        stack_name,
        HashSet::from([output_key.to_string()]),
    )
    .await?
    .into_values()
    .next()
    .unwrap())
}

pub async fn deploy_stack_from_s3(
    pr: &Printer,
    profile: &str,
    stack_name: &str,
    stack_region: &str,
    s3_bucket: &str,
    s3_region: &str,
    s3_key: &str,
    method: StackDeploymentMethod,
    parameters: HashMap<String, String>,
) -> Result<(), CliError> {
    let client = Client::new(&config_from_profile(profile, stack_region).await);

    let s3_url = format!("https://{s3_bucket}.s3.{s3_region}.amazonaws.com/{s3_key}");
    let parameters = parameters
        .into_iter()
        .map(|(key, value)| {
            Parameter::builder()
                .parameter_key(key)
                .parameter_value(value)
                .build()
        })
        .collect::<Vec<_>>();
    match method {
        StackDeploymentMethod::Changeset => {
            pr.info(&format!(
                "Creating CloudFormation changeset for stack '{}' from S3 URL '{}'...",
                stack_name, s3_url
            ));
            let changeset_name = chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
            client
                .create_change_set()
                .stack_name(stack_name)
                .change_set_name(&changeset_name)
                .template_url(s3_url)
                .capabilities(Capability::CapabilityNamedIam)
                .include_nested_stacks(true)
                .set_parameters(Some(parameters))
                .send()
                .await
                .map_err(|e| CloudFormationDeploymentFailed::with_debug(stack_name, &e))?;
            pr.important(&format!("Created changeset '{}'.", changeset_name));
        }
        StackDeploymentMethod::Direct => {
            pr.info(&format!(
                "Deploying CloudFormation stack '{}' from S3 URL '{}'...",
                stack_name, s3_url
            ));
            client
                .create_stack()
                .stack_name(stack_name)
                .template_url(s3_url)
                .capabilities(Capability::CapabilityNamedIam)
                .set_parameters(Some(parameters))
                .send()
                .await
                .map_err(|e| CloudFormationDeploymentFailed::with_debug(stack_name, &e))?;
            pr.info("Deployment initiated. Waiting...");
            client
                .wait_until_stack_create_complete()
                .stack_name(stack_name)
                .wait(DEPLOY_WAIT_TIMEOUT)
                .await
                .map_err(|e| CloudFormationDeploymentFailed::with_debug(stack_name, &e))?;
            pr.important("Deployment complete.");
        }
    }

    Ok(())
}
