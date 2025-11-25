//! AWS ECS cluster provider for task discovery-based clustering.
//!
//! This provider discovers ECS tasks via the ECS API (ListTasks + DescribeTasks)
//! and publishes topology updates when tasks join or leave the cluster.
//!
//! # Features
//!
//! - Automatic task discovery via ECS API polling
//! - Support for both EC2 and Fargate launch types
//! - awsvpc network mode with private IP extraction
//! - Background polling with configurable interval
//!
//! # Required IAM Permissions
//!
//! ```json
//! {
//!   "Effect": "Allow",
//!   "Action": ["ecs:ListTasks", "ecs:DescribeTasks"],
//!   "Resource": "*"
//! }
//! ```
//!
//! # Example
//!
//! ```ignore
//! use fraktor_cluster_rs::std::EcsClusterConfig;
//! use fraktor_cluster_rs::core::ClusterExtensionInstaller;
//!
//! let ecs_config = EcsClusterConfig::new()
//!     .with_cluster_name("my-cluster")
//!     .with_service_name("my-service")
//!     .with_poll_interval(Duration::from_secs(10));
//!
//! let installer = ClusterExtensionInstaller::new_with_ecs(config, ecs_config);
//! ```

#[cfg(test)]
mod tests;

use std::{
  string::ToString,
  sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
  },
  time::Duration,
};

use aws_sdk_ecs::Client as EcsClient;
use fraktor_actor_rs::core::{
  event_stream::{EventStreamEvent, EventStreamGeneric},
  messaging::AnyMessageGeneric,
};
use fraktor_remote_rs::core::BlockListProvider;
use fraktor_utils_rs::{core::sync::ArcShared, std::runtime_toolbox::StdToolbox};
use tokio::task::JoinHandle;

use crate::core::{ClusterEvent, ClusterProvider, ClusterProviderError, ClusterTopology, StartupMode};

/// Configuration for AWS ECS cluster provider.
#[derive(Clone, Debug)]
pub struct EcsClusterConfig {
  cluster_name:  String,
  service_name:  Option<String>,
  poll_interval: Duration,
  port:          u16,
  region:        Option<String>,
}

impl Default for EcsClusterConfig {
  fn default() -> Self {
    Self::new()
  }
}

impl EcsClusterConfig {
  /// Creates a new ECS cluster configuration with default values.
  #[must_use]
  pub fn new() -> Self {
    Self {
      cluster_name:  String::new(),
      service_name:  None,
      poll_interval: Duration::from_secs(30),
      port:          8080,
      region:        None,
    }
  }

  /// Sets the ECS cluster name.
  #[must_use]
  pub fn with_cluster_name(mut self, name: impl Into<String>) -> Self {
    self.cluster_name = name.into();
    self
  }

  /// Sets the ECS service name for filtering tasks.
  #[must_use]
  pub fn with_service_name(mut self, name: impl Into<String>) -> Self {
    self.service_name = Some(name.into());
    self
  }

  /// Sets the polling interval for task discovery.
  #[must_use]
  pub const fn with_poll_interval(mut self, interval: Duration) -> Self {
    self.poll_interval = interval;
    self
  }

  /// Sets the port used for cluster communication.
  #[must_use]
  pub const fn with_port(mut self, port: u16) -> Self {
    self.port = port;
    self
  }

  /// Sets the AWS region.
  #[must_use]
  pub fn with_region(mut self, region: impl Into<String>) -> Self {
    self.region = Some(region.into());
    self
  }

  /// Returns the cluster name.
  #[must_use]
  pub fn cluster_name(&self) -> &str {
    &self.cluster_name
  }

  /// Returns the service name.
  #[must_use]
  pub fn service_name(&self) -> Option<&str> {
    self.service_name.as_deref()
  }

  /// Returns the polling interval.
  #[must_use]
  pub const fn poll_interval(&self) -> Duration {
    self.poll_interval
  }

  /// Returns the port.
  #[must_use]
  pub const fn port(&self) -> u16 {
    self.port
  }

  /// Returns the region.
  #[must_use]
  pub fn region(&self) -> Option<&str> {
    self.region.as_deref()
  }
}

/// AWS ECS cluster provider that discovers tasks via ECS API.
///
/// This provider polls the ECS API to discover running tasks and publishes
/// topology updates when tasks join or leave the cluster.
pub struct AwsEcsClusterProvider {
  event_stream:        ArcShared<EventStreamGeneric<StdToolbox>>,
  block_list_provider: ArcShared<dyn BlockListProvider>,
  advertised_address:  String,
  config:              EcsClusterConfig,
  members:             Mutex<Vec<String>>,
  version:             Mutex<u64>,
  startup_mode:        Mutex<Option<StartupMode>>,
  shutdown_flag:       ArcShared<AtomicBool>,
  poller_handle:       Mutex<Option<JoinHandle<()>>>,
}

impl AwsEcsClusterProvider {
  /// Creates a new AWS ECS cluster provider.
  #[must_use]
  pub fn new(
    event_stream: ArcShared<EventStreamGeneric<StdToolbox>>,
    block_list_provider: ArcShared<dyn BlockListProvider>,
    advertised_address: impl Into<String>,
  ) -> Self {
    Self {
      event_stream,
      block_list_provider,
      advertised_address: advertised_address.into(),
      config: EcsClusterConfig::new(),
      members: Mutex::new(Vec::new()),
      version: Mutex::new(0),
      startup_mode: Mutex::new(None),
      shutdown_flag: ArcShared::new(AtomicBool::new(false)),
      poller_handle: Mutex::new(None),
    }
  }

  /// Sets the ECS cluster configuration.
  #[must_use]
  pub fn with_ecs_config(mut self, config: EcsClusterConfig) -> Self {
    self.config = config;
    self
  }

  /// Returns the advertised address.
  #[must_use]
  pub fn advertised_address(&self) -> &str {
    &self.advertised_address
  }

  /// Returns whether the provider has been started.
  #[must_use]
  pub fn is_started(&self) -> bool {
    self.startup_mode.lock().unwrap().is_some()
  }

  /// Returns the current member count.
  #[must_use]
  pub fn member_count(&self) -> usize {
    self.members.lock().unwrap().len()
  }

  fn next_version(&self) -> u64 {
    let mut version = self.version.lock().unwrap();
    *version += 1;
    *version
  }

  fn publish_topology(&self, version: u64, joined: Vec<String>, left: Vec<String>) {
    let blocked = self.block_list_provider.blocked_members();
    let topology = ClusterTopology::new(version, joined.clone(), left.clone());
    let event = ClusterEvent::TopologyUpdated { topology, joined, left, blocked };
    let payload = AnyMessageGeneric::new(event);
    let extension_event = EventStreamEvent::Extension { name: String::from("cluster"), payload };
    self.event_stream.publish(&extension_event);
  }

  fn publish_startup_event(&self, mode: StartupMode) {
    let event = ClusterEvent::Startup { address: self.advertised_address.clone(), mode };
    let payload = AnyMessageGeneric::new(event);
    let extension_event = EventStreamEvent::Extension { name: String::from("cluster"), payload };
    self.event_stream.publish(&extension_event);
  }

  fn publish_shutdown_event(&self, mode: StartupMode) {
    let event = ClusterEvent::Shutdown { address: self.advertised_address.clone(), mode };
    let payload = AnyMessageGeneric::new(event);
    let extension_event = EventStreamEvent::Extension { name: String::from("cluster"), payload };
    self.event_stream.publish(&extension_event);
  }

  fn start_polling(&self, add_self: bool) {
    let shutdown_flag = ArcShared::clone(&self.shutdown_flag);
    let event_stream = ArcShared::clone(&self.event_stream);
    let block_list_provider = ArcShared::clone(&self.block_list_provider);
    let config = self.config.clone();
    let advertised_address = self.advertised_address.clone();

    let handle = tokio::spawn(async move {
      // AWS SDK クライアントを初期化
      let aws_config = if let Some(ref region) = config.region {
        aws_config::from_env().region(aws_sdk_ecs::config::Region::new(region.clone())).load().await
      } else {
        aws_config::load_from_env().await
      };
      let ecs_client = EcsClient::new(&aws_config);

      let mut current_members: Vec<String> = if add_self { vec![advertised_address.clone()] } else { Vec::new() };
      let mut version: u64 = 0;

      loop {
        if (*shutdown_flag).load(Ordering::Relaxed) {
          break;
        }

        // ECS タスクをポーリング
        match poll_ecs_tasks(&ecs_client, &config).await {
          | Ok(discovered_ips) => {
            // 新しいメンバーリストを構築（自分自身を含める場合）
            let port = config.port;
            let mut new_members: Vec<String> =
              discovered_ips.into_iter().map(|ip| format!("{}:{}", ip, port)).collect();

            if add_self && !new_members.contains(&advertised_address) {
              new_members.push(advertised_address.clone());
            }

            // 差分を計算
            let joined: Vec<String> = new_members.iter().filter(|m| !current_members.contains(m)).cloned().collect();
            let left: Vec<String> = current_members.iter().filter(|m| !new_members.contains(m)).cloned().collect();

            // 変更があればトポロジ更新をパブリッシュ
            if !joined.is_empty() || !left.is_empty() {
              version += 1;
              let blocked = block_list_provider.blocked_members();
              let topology = ClusterTopology::new(version, joined.clone(), left.clone());
              let event = ClusterEvent::TopologyUpdated { topology, joined, left, blocked };
              let payload = AnyMessageGeneric::new(event);
              let extension_event = EventStreamEvent::Extension { name: String::from("cluster"), payload };
              event_stream.publish(&extension_event);

              current_members = new_members;
            }
          },
          | Err(_e) => {
            // ポーリングエラーは無視して次のサイクルを待つ（MVP実装）
            // 本番環境ではログ出力やリトライロジックを追加
          },
        }

        tokio::time::sleep(config.poll_interval).await;
      }
    });

    *self.poller_handle.lock().unwrap() = Some(handle);
  }
}

/// Polls ECS tasks and returns their private IPs.
async fn poll_ecs_tasks(client: &EcsClient, config: &EcsClusterConfig) -> Result<Vec<String>, EcsPollerError> {
  // ListTasks API 呼び出し
  let mut list_tasks_req = client.list_tasks().cluster(&config.cluster_name);

  if let Some(ref service_name) = config.service_name {
    list_tasks_req = list_tasks_req.service_name(service_name);
  }

  let list_tasks_resp = list_tasks_req.send().await.map_err(|e| EcsPollerError::ApiCall(e.to_string()))?;

  let task_arns = list_tasks_resp.task_arns();
  if task_arns.is_empty() {
    return Ok(Vec::new());
  }

  // DescribeTasks API 呼び出し（最大100タスク/リクエスト）
  let describe_tasks_resp = client
    .describe_tasks()
    .cluster(&config.cluster_name)
    .set_tasks(Some(task_arns.to_vec()))
    .send()
    .await
    .map_err(|e| EcsPollerError::ApiCall(e.to_string()))?;

  // RUNNING タスクからプライベート IP を抽出
  let private_ips: Vec<String> = describe_tasks_resp
    .tasks()
    .iter()
    .filter(|task| task.last_status() == Some("RUNNING"))
    .filter_map(|task| extract_private_ip(task))
    .collect();

  Ok(private_ips)
}

/// Extracts the private IP address from an ECS task's awsvpc attachment.
fn extract_private_ip(task: &aws_sdk_ecs::types::Task) -> Option<String> {
  task.attachments().iter().find(|a| a.r#type() == Some("ElasticNetworkInterface")).and_then(|eni| {
    eni.details().iter().find(|d| d.name() == Some("privateIPv4Address")).and_then(|d| d.value().map(String::from))
  })
}

/// Error type for ECS polling operations.
#[derive(Debug)]
pub enum EcsPollerError {
  /// API call failed.
  ApiCall(String),
}

impl ClusterProvider for AwsEcsClusterProvider {
  fn start_member(&self) -> Result<(), ClusterProviderError> {
    // 起動モードを設定
    *self.startup_mode.lock().unwrap() = Some(StartupMode::Member);

    // 自分自身をメンバーリストに追加
    {
      let mut members = self.members.lock().unwrap();
      if !members.contains(&self.advertised_address) {
        members.push(self.advertised_address.clone());
      }
    }

    // 初回トポロジを publish
    let version = self.next_version();
    self.publish_topology(version, vec![self.advertised_address.clone()], vec![]);

    // バックグラウンドポーリングを開始
    self.start_polling(true);

    // Startup イベントを発火
    self.publish_startup_event(StartupMode::Member);

    Ok(())
  }

  fn start_client(&self) -> Result<(), ClusterProviderError> {
    // 起動モードを設定
    *self.startup_mode.lock().unwrap() = Some(StartupMode::Client);

    // バックグラウンドポーリングを開始（自身は追加しない）
    self.start_polling(false);

    // Startup イベントを発火
    self.publish_startup_event(StartupMode::Client);

    Ok(())
  }

  fn shutdown(&self, _graceful: bool) -> Result<(), ClusterProviderError> {
    // シャットダウンフラグを設定
    self.shutdown_flag.store(true, Ordering::Relaxed);

    // 起動モードを取得してからクリア
    let mode = self.startup_mode.lock().unwrap().take().unwrap_or(StartupMode::Member);

    // メンバーリストをクリア
    {
      let mut members = self.members.lock().unwrap();
      members.clear();
    }

    // Shutdown イベントを発火
    self.publish_shutdown_event(mode);

    Ok(())
  }
}
