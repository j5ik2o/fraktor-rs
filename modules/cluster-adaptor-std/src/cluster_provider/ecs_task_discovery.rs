//! ECS task discovery helpers.

use std::string::{String, ToString};

use aws_sdk_ecs::{Client as EcsClient, types::Task};

use super::{EcsClusterConfig, EcsPollerError};

pub(super) async fn poll_ecs_tasks(
  client: &EcsClient,
  config: &EcsClusterConfig,
) -> Result<Vec<String>, EcsPollerError> {
  let mut list_tasks_req = client.list_tasks().cluster(config.cluster_name());

  if let Some(service_name) = config.service_name() {
    list_tasks_req = list_tasks_req.service_name(service_name);
  }

  let list_tasks_resp = list_tasks_req.send().await.map_err(|err| EcsPollerError::ApiCall(err.to_string()))?;

  let task_arns = list_tasks_resp.task_arns();
  if task_arns.is_empty() {
    return Ok(Vec::new());
  }

  let describe_tasks_resp = client
    .describe_tasks()
    .cluster(config.cluster_name())
    .set_tasks(Some(task_arns.to_vec()))
    .send()
    .await
    .map_err(|err| EcsPollerError::ApiCall(err.to_string()))?;

  let private_ips: Vec<String> = describe_tasks_resp
    .tasks()
    .iter()
    .filter(|task| task.last_status() == Some("RUNNING"))
    .filter_map(extract_private_ip)
    .collect();

  Ok(private_ips)
}

fn extract_private_ip(task: &Task) -> Option<String> {
  task.attachments().iter().find(|attachment| attachment.r#type() == Some("ElasticNetworkInterface")).and_then(
    |attachment| {
      attachment
        .details()
        .iter()
        .find(|detail| detail.name() == Some("privateIPv4Address"))
        .and_then(|detail| detail.value().map(String::from))
    },
  )
}
