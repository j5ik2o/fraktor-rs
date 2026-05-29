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
#[cfg(test)]
#[path = "aws_ecs_cluster_provider_test.rs"]
mod tests;

use std::{
  string::ToString,
  sync::atomic::{AtomicBool, Ordering},
  time::Duration,
};

use fraktor_actor_core_kernel_rs::{
  actor::messaging::AnyMessage,
  event::stream::{EventStreamEvent, EventStreamShared},
};
use fraktor_cluster_core_kernel_rs::{
  cluster_provider::ClusterProvider,
  extension::{ClusterProviderError, StartupMode},
  topology::{BlockListProvider, ClusterEvent, ClusterTopology, TopologyUpdate},
};
use fraktor_utils_core_rs::{sync::ArcShared, time::TimerInstant};
use tokio::task::JoinHandle;

use super::{EcsClusterConfig, ecs_task_discovery::poll_ecs_tasks};

/// AWS ECS cluster provider that discovers tasks via ECS API.
///
/// This provider polls the ECS API to discover running tasks and publishes
/// topology updates when tasks join or leave the cluster.
pub struct AwsEcsClusterProvider {
  event_stream:        EventStreamShared,
  block_list_provider: ArcShared<dyn BlockListProvider>,
  advertised_address:  String,
  config:              EcsClusterConfig,
  members:             Vec<String>,
  version:             u64,
  startup_mode:        Option<StartupMode>,
  shutdown_flag:       ArcShared<AtomicBool>,
  poller_handle:       Option<JoinHandle<()>>,
}

impl AwsEcsClusterProvider {
  /// Creates a new AWS ECS cluster provider.
  #[must_use]
  pub fn new(
    event_stream: EventStreamShared,
    block_list_provider: ArcShared<dyn BlockListProvider>,
    advertised_address: impl Into<String>,
  ) -> Self {
    Self {
      event_stream,
      block_list_provider,
      advertised_address: advertised_address.into(),
      config: EcsClusterConfig::new(),
      members: Vec::new(),
      version: 0,
      startup_mode: None,
      shutdown_flag: ArcShared::new(AtomicBool::new(false)),
      poller_handle: None,
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
    self.startup_mode.is_some()
  }

  /// Returns the current member count.
  #[must_use]
  pub fn member_count(&self) -> usize {
    self.members.len()
  }

  fn next_version(&mut self) -> u64 {
    self.version += 1;
    self.version
  }

  fn publish_topology(&self, version: u64, joined: Vec<String>, left: Vec<String>) {
    let blocked = self.block_list_provider.blocked_members();
    let topology = ClusterTopology::new(version, joined.clone(), left.clone(), Vec::new());
    let update =
      TopologyUpdate::new(topology, self.members.clone(), joined, left, Vec::new(), blocked, self.observed_at(version));
    let event = ClusterEvent::TopologyUpdated { update };
    let payload = AnyMessage::new(event);
    let extension_event = EventStreamEvent::Extension { name: String::from("cluster"), payload };
    self.event_stream.publish(&extension_event);
  }

  fn publish_startup_event(&self, mode: StartupMode) {
    let event = ClusterEvent::Startup { address: self.advertised_address.clone(), mode };
    let payload = AnyMessage::new(event);
    let extension_event = EventStreamEvent::Extension { name: String::from("cluster"), payload };
    self.event_stream.publish(&extension_event);
  }

  fn publish_shutdown_event(&self, mode: StartupMode) {
    let event = ClusterEvent::Shutdown { address: self.advertised_address.clone(), mode };
    let payload = AnyMessage::new(event);
    let extension_event = EventStreamEvent::Extension { name: String::from("cluster"), payload };
    self.event_stream.publish(&extension_event);
  }

  fn observed_at(&self, version: u64) -> TimerInstant {
    TimerInstant::from_ticks(version, Duration::from_secs(1))
  }

  fn start_polling(&mut self, add_self: bool) {
    let shutdown_flag = ArcShared::clone(&self.shutdown_flag);
    let event_stream = self.event_stream.clone();
    let block_list_provider = ArcShared::clone(&self.block_list_provider);
    let config = self.config.clone();
    let advertised_address = self.advertised_address.clone();

    let handle = tokio::spawn(async move {
      let ecs_client = config.create_client().await;

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
            let port = config.port();
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
              let topology = ClusterTopology::new(version, joined.clone(), left.clone(), Vec::new());
              let update = TopologyUpdate::new(
                topology,
                new_members.clone(),
                joined,
                left,
                Vec::new(),
                blocked,
                TimerInstant::from_ticks(version, Duration::from_secs(1)),
              );
              let event = ClusterEvent::TopologyUpdated { update };
              let payload = AnyMessage::new(event);
              let extension_event = EventStreamEvent::Extension { name: String::from("cluster"), payload };
              event_stream.publish(&extension_event);

              current_members = new_members;
            }
          },
          | Err(error) => {
            tracing::warn!(
              cluster = %config.cluster_name(),
              service = ?config.service_name(),
              "ECS polling failed: {error:?}"
            );
          },
        }

        tokio::time::sleep(config.poll_interval()).await;
      }
    });

    self.poller_handle = Some(handle);
  }
}

impl ClusterProvider for AwsEcsClusterProvider {
  fn start_member(&mut self) -> Result<(), ClusterProviderError> {
    // 起動モードを設定
    self.startup_mode = Some(StartupMode::Member);

    // 自分自身をメンバーリストに追加
    if !self.members.contains(&self.advertised_address) {
      self.members.push(self.advertised_address.clone());
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

  fn start_client(&mut self) -> Result<(), ClusterProviderError> {
    // 起動モードを設定
    self.startup_mode = Some(StartupMode::Client);

    // バックグラウンドポーリングを開始（自身は追加しない）
    self.start_polling(false);

    // Startup イベントを発火
    self.publish_startup_event(StartupMode::Client);

    Ok(())
  }

  fn down(&mut self, authority: &str) -> Result<(), ClusterProviderError> {
    if authority == self.advertised_address {
      return Err(ClusterProviderError::down("cannot down self authority"));
    }
    if !self.members.contains(&String::from(authority)) {
      return Ok(());
    }
    self.members.retain(|member| member != authority);
    let version = self.next_version();
    self.publish_topology(version, vec![], vec![authority.to_string()]);
    Ok(())
  }

  fn join(&mut self, _authority: &str) -> Result<(), ClusterProviderError> {
    Err(ClusterProviderError::join("join is not supported by aws ecs provider"))
  }

  fn leave(&mut self, _authority: &str) -> Result<(), ClusterProviderError> {
    Err(ClusterProviderError::leave("leave is not supported by aws ecs provider"))
  }

  fn shutdown(&mut self, _graceful: bool) -> Result<(), ClusterProviderError> {
    // シャットダウンフラグを設定
    self.shutdown_flag.store(true, Ordering::Relaxed);

    // 起動モードを取得してからクリア
    let mode = self.startup_mode.take().unwrap_or(StartupMode::Member);

    // メンバーリストをクリア
    self.members.clear();

    // Shutdown イベントを発火
    self.publish_shutdown_event(mode);

    Ok(())
  }
}
