use aws_sdk_ecs::Client;
use lib_core::{define_cli_error, CliError, Printer};

use crate::{set_auto_scaling_group_desired_size, shared_config::config_from_profile};

define_cli_error!(EcsError, "Error running AWS ECS command.");
define_cli_error!(EcsCapacityProviderNotFound, "ECS capacity provider '{name}' not found.", { name: &str });
define_cli_error!(EcsCapacityProviderNoAutoScalingGroup, "ECS capacity provider '{name}' does not have an auto scaling group.", { name: &str });

pub async fn cluster_has_running_task_for_family(
    profile: &str,
    region: &str,
    cluster: &str,
    task_definition_family: &str,
) -> Result<bool, CliError> {
    let client = Client::new(&config_from_profile(profile, region).await);
    let response = client
        .list_tasks()
        .cluster(cluster)
        .family(task_definition_family)
        .send()
        .await
        .map_err(|e| EcsError::with_debug(&e))?;
    Ok(!response.task_arns.unwrap_or_default().is_empty())
}

pub async fn run_task(
    pr: &Printer,
    profile: &str,
    region: &str,
    cluster: &str,
    task_definition: &str,
) -> Result<(), CliError> {
    pr.info(&format!(
        "Running task '{}' on cluster '{}'...",
        task_definition, cluster
    ));
    let client = Client::new(&config_from_profile(profile, region).await);
    client
        .run_task()
        .cluster(cluster)
        .task_definition(task_definition)
        .send()
        .await
        .map_err(|e| EcsError::with_debug(&e))?;
    Ok(())
}

/// This assumes the task definition's family is the same as the task definition name.
pub async fn run_task_if_not_running(
    pr: &Printer,
    profile: &str,
    region: &str,
    cluster: &str,
    task_definition: &str,
) -> Result<(), CliError> {
    if !cluster_has_running_task_for_family(profile, region, cluster, task_definition).await? {
        run_task(pr, profile, region, cluster, task_definition).await?;
    }
    Ok(())
}

/// Updates the desired size of the EC2 auto scaling group associated with the
/// given capacity provider.
pub async fn set_capacity_provider_desired_size(
    pr: &Printer,
    profile: &str,
    region: &str,
    capacity_provider: &str,
    desired_size: i32,
) -> Result<(), CliError> {
    let client = Client::new(&config_from_profile(profile, region).await);
    let auto_scaling_group = client
        .describe_capacity_providers()
        .send()
        .await
        .map_err(|e| EcsError::with_debug(&e))?
        .capacity_providers
        .unwrap_or_default()
        .iter()
        .find(|cp| matches!(cp.name, Some(ref name) if name == capacity_provider))
        .ok_or_else(|| EcsCapacityProviderNotFound::new(capacity_provider))?
        .auto_scaling_group_provider
        .as_ref()
        .ok_or_else(|| EcsCapacityProviderNoAutoScalingGroup::new(capacity_provider))?
        .auto_scaling_group_arn
        .clone();
    set_auto_scaling_group_desired_size(pr, profile, region, &auto_scaling_group, desired_size)
        .await
}
