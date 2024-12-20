use aws_sdk_ecs::{
    types::{AssignPublicIp, AwsVpcConfiguration, LaunchType, NetworkConfiguration},
    Client,
};
use lib_core::{define_cli_error, CliError, Printer};

use crate::{
    get_auto_scaling_group_name_from_arn, set_auto_scaling_group_desired_size,
    shared_config::config_from_profile,
};

define_cli_error!(EcsError, "Error running AWS ECS command.");
define_cli_error!(EcsCapacityProviderNotFound, "ECS capacity provider '{name}' not found.", { name: &str });
define_cli_error!(EcsCapacityProviderNoAutoScalingGroup, "ECS capacity provider '{name}' does not have an auto scaling group.", { name: &str });

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EcsTaskLaunchType {
    Ec2,
    Fargate,
}

#[derive(Debug, Clone)]
pub struct EcsTaskNetworkConfiguration {
    pub subnet_id: Option<String>,
    pub security_group_id: Option<String>,
    pub assign_public_ip: Option<bool>,
}

impl Into<LaunchType> for EcsTaskLaunchType {
    fn into(self) -> LaunchType {
        match self {
            EcsTaskLaunchType::Ec2 => LaunchType::Ec2,
            EcsTaskLaunchType::Fargate => LaunchType::Fargate,
        }
    }
}

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
    launch_type: Option<EcsTaskLaunchType>,
    network_configuration: Option<EcsTaskNetworkConfiguration>,
) -> Result<(), CliError> {
    pr.info(&format!(
        "Running task '{}' on cluster '{}' ({})...",
        task_definition, cluster, region
    ));
    let client = Client::new(&config_from_profile(profile, region).await);
    client
        .run_task()
        .cluster(cluster)
        .task_definition(task_definition)
        .set_launch_type(launch_type.map(Into::into))
        .set_network_configuration(
            network_configuration
                .map(|nc| {
                    Ok::<_, CliError>(
                        NetworkConfiguration::builder()
                            .awsvpc_configuration(
                                AwsVpcConfiguration::builder()
                                    .set_subnets(nc.subnet_id.map(|s| vec![s]))
                                    .set_security_groups(nc.security_group_id.map(|s| vec![s]))
                                    .set_assign_public_ip(match nc.assign_public_ip {
                                        Some(true) => Some(AssignPublicIp::Enabled),
                                        Some(false) => Some(AssignPublicIp::Disabled),
                                        None => None,
                                    })
                                    .build()
                                    .map_err(|e| EcsError::with_debug(&e))?,
                            )
                            .build(),
                    )
                })
                .transpose()?,
        )
        .send()
        .await
        .map_err(|e| EcsError::with_debug(&e))?;
    Ok(())
}

/// This assumes the task definition's family is the same as the task definition
/// name.
///
/// Returns true if a new task was started.
pub async fn run_task_if_not_running(
    pr: &Printer,
    profile: &str,
    region: &str,
    cluster: &str,
    task_definition: &str,
    launch_type: Option<EcsTaskLaunchType>,
    network_configuration: Option<EcsTaskNetworkConfiguration>,
) -> Result<bool, CliError> {
    if !cluster_has_running_task_for_family(profile, region, cluster, task_definition).await? {
        run_task(
            pr,
            profile,
            region,
            cluster,
            task_definition,
            launch_type,
            network_configuration,
        )
        .await?;
        Ok(true)
    } else {
        Ok(false)
    }
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
    let auto_scaling_group_arn = client
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
    let auto_scaling_group_name = get_auto_scaling_group_name_from_arn(&auto_scaling_group_arn);
    set_auto_scaling_group_desired_size(pr, profile, region, &auto_scaling_group_name, desired_size)
        .await
}

pub async fn stop_all_tasks(
    pr: &Printer,
    profile: &str,
    region: &str,
    cluster: &str,
) -> Result<(), CliError> {
    pr.info(&format!(
        "Stopping all tasks on cluster '{}' ({})...",
        cluster, region
    ));
    let client = Client::new(&config_from_profile(profile, region).await);
    let tasks = client
        .list_tasks()
        .cluster(cluster)
        .send()
        .await
        .map_err(|e| EcsError::with_debug(&e))?
        .task_arns
        .unwrap_or_default();
    for task in tasks {
        client
            .stop_task()
            .cluster(cluster)
            .task(task)
            .send()
            .await
            .map_err(|e| EcsError::with_debug(&e))?;
    }
    Ok(())
}
