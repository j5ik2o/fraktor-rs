//! Local cluster provider for membership-aware cluster scenarios (no_std compatible).
//!
//! This provider publishes ClusterTopology events to EventStream based on
//! membership changes. The core implementation is no_std compatible, while
//! transport event subscription is available as an optional std feature.
//!
//! Phase2 Task 4.5: Transport connection/disconnection event auto-detection
//! for TopologyUpdated publishing is implemented via conditional compilation
//! and only available in std environments.

use alloc::{string::String, vec::Vec};

use fraktor_actor_rs::core::{
  event_stream::{EventStreamEvent, EventStreamGeneric},
  messaging::AnyMessageGeneric,
};
use fraktor_remote_rs::core::BlockListProvider;
use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};

use crate::core::{ClusterEvent, ClusterProvider, ClusterProviderError, ClusterTopology, StartupMode};

#[cfg(test)]
mod tests;

/// Local cluster provider that publishes topology events to EventStream.
///
/// This provider manages membership state and publishes `ClusterEvent::TopologyUpdated`
/// events when nodes join or leave the cluster. It serves as a reference implementation
/// for TCP-based cluster providers like etcd, zk, or automanaged providers.
///
/// The core implementation is no_std compatible using `RuntimeToolbox` for
/// synchronization primitives.
///
/// Phase2 features like seed_nodes for GossipEngine initialization and
/// startup/shutdown events are fully supported.
///
/// Task 4.5: Transport `RemotingLifecycleEvent::Connected` and `Quarantined`
/// auto-detection is available via `subscribe_remoting_events()` in std environments.
pub struct LocalClusterProvider<TB: RuntimeToolbox + 'static> {
  event_stream:        ArcShared<EventStreamGeneric<TB>>,
  block_list_provider: ArcShared<dyn BlockListProvider>,
  advertised_address:  String,
  // 現在のメンバーリスト（join/leave イベント処理用）
  members:             ToolboxMutex<Vec<String>, TB>,
  // 内部バージョンカウンタ（ハッシュ生成用）
  version:             ToolboxMutex<u64, TB>,
  // 静的トポロジ（設定されている場合、start時に publish）
  static_topology:     ToolboxMutex<Option<ClusterTopology>, TB>,
  // GossipEngine 用の seed ノードリスト（Phase2）
  seed_nodes:          ToolboxMutex<Vec<String>, TB>,
  // 起動モード（Member/Client）を追跡
  startup_mode:        ToolboxMutex<Option<StartupMode>, TB>,
}

impl<TB: RuntimeToolbox + 'static> LocalClusterProvider<TB> {
  /// Creates a new local cluster provider.
  #[must_use]
  pub fn new(
    event_stream: ArcShared<EventStreamGeneric<TB>>,
    block_list_provider: ArcShared<dyn BlockListProvider>,
    advertised_address: impl Into<String>,
  ) -> Self {
    Self {
      event_stream,
      block_list_provider,
      advertised_address: advertised_address.into(),
      members: <TB::MutexFamily as SyncMutexFamily>::create(Vec::new()),
      version: <TB::MutexFamily as SyncMutexFamily>::create(0),
      static_topology: <TB::MutexFamily as SyncMutexFamily>::create(None),
      seed_nodes: <TB::MutexFamily as SyncMutexFamily>::create(Vec::new()),
      startup_mode: <TB::MutexFamily as SyncMutexFamily>::create(None),
    }
  }

  /// Sets a static topology to be published on startup.
  ///
  /// This is useful for testing or scenarios where topology is predetermined.
  #[must_use]
  pub fn with_static_topology(self, topology: ClusterTopology) -> Self {
    *self.static_topology.lock() = Some(topology);
    self
  }

  /// Sets the seed nodes for GossipEngine initialization.
  ///
  /// These nodes will be used as initial peers when the provider starts.
  /// In Phase2, this enables GossipEngine to establish connections with known peers.
  #[must_use]
  pub fn with_seed_nodes(self, seeds: Vec<String>) -> Self {
    *self.seed_nodes.lock() = seeds;
    self
  }

  /// Returns the advertised address.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)]
  pub fn advertised_address(&self) -> &str {
    &self.advertised_address
  }

  /// Returns the configured seed nodes.
  #[must_use]
  pub fn seed_nodes(&self) -> Vec<String> {
    self.seed_nodes.lock().clone()
  }

  /// Notifies the provider that a node has joined the cluster.
  ///
  /// This will publish a `ClusterEvent::TopologyUpdated` with the joined node
  /// in the `joined` list.
  pub fn on_member_join(&self, authority: impl Into<String>) {
    let authority = authority.into();
    let mut members = self.members.lock();
    if !members.contains(&authority) {
      members.push(authority.clone());
    }
    drop(members);

    let version = self.next_version();
    self.publish_topology(version, alloc::vec![authority], alloc::vec![]);
  }

  /// Notifies the provider that a node has left the cluster.
  ///
  /// This will publish a `ClusterEvent::TopologyUpdated` with the left node
  /// in the `left` list.
  pub fn on_member_leave(&self, authority: impl Into<String>) {
    let authority = authority.into();
    let mut members = self.members.lock();
    members.retain(|m| m != &authority);
    drop(members);

    let version = self.next_version();
    self.publish_topology(version, alloc::vec![], alloc::vec![authority]);
  }

  /// Returns the current member count.
  #[must_use]
  pub fn member_count(&self) -> usize {
    self.members.lock().len()
  }

  /// Returns whether the provider has been started.
  #[must_use]
  pub fn is_started(&self) -> bool {
    self.startup_mode.lock().is_some()
  }

  /// Returns the event stream reference.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)]
  pub fn event_stream(&self) -> &ArcShared<EventStreamGeneric<TB>> {
    &self.event_stream
  }

  fn next_version(&self) -> u64 {
    let mut version = self.version.lock();
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

  fn publish_static_topology(&self) {
    let static_topology = self.static_topology.lock();
    if let Some(topology) = static_topology.as_ref() {
      let blocked = self.block_list_provider.blocked_members();
      let event = ClusterEvent::TopologyUpdated {
        topology: topology.clone(),
        joined: topology.joined().clone(),
        left: topology.left().clone(),
        blocked,
      };
      let payload = AnyMessageGeneric::new(event);
      let extension_event = EventStreamEvent::Extension { name: String::from("cluster"), payload };
      self.event_stream.publish(&extension_event);
    }
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

  /// Handles a connected event from remoting, triggering a member join.
  ///
  /// This is called internally when transport connection events are detected.
  /// Can also be used for manual integration with custom transport implementations.
  pub fn handle_connected(&self, authority: &str) {
    // 自分自身の authority は無視（既に members に含まれているはず）
    if authority == self.advertised_address {
      return;
    }
    // メンバーリストに追加されていない場合のみ join イベントを発火
    let should_join = {
      let members = self.members.lock();
      !members.contains(&String::from(authority))
    };
    if should_join {
      self.on_member_join(authority);
    }
  }

  /// Handles a quarantined event from remoting, triggering a member leave.
  ///
  /// This is called internally when transport quarantine events are detected.
  /// Can also be used for manual integration with custom transport implementations.
  pub fn handle_quarantined(&self, authority: &str) {
    // 自分自身の authority は無視
    if authority == self.advertised_address {
      return;
    }
    // メンバーリストに含まれている場合のみ leave イベントを発火
    let should_leave = {
      let members = self.members.lock();
      members.contains(&String::from(authority))
    };
    if should_leave {
      self.on_member_leave(authority);
    }
  }
}

impl<TB: RuntimeToolbox + 'static> ClusterProvider for LocalClusterProvider<TB> {
  fn start_member(&self) -> Result<(), ClusterProviderError> {
    // 起動モードを設定
    *self.startup_mode.lock() = Some(StartupMode::Member);

    // 自分自身をメンバーリストに追加
    {
      let mut members = self.members.lock();
      if !members.contains(&self.advertised_address) {
        members.push(self.advertised_address.clone());
      }
    }

    // 静的トポロジが設定されている場合は publish
    self.publish_static_topology();

    // Startup イベントを EventStream に発火
    self.publish_startup_event(StartupMode::Member);

    Ok(())
  }

  fn start_client(&self) -> Result<(), ClusterProviderError> {
    // 起動モードを設定
    *self.startup_mode.lock() = Some(StartupMode::Client);

    // クライアントモードでも静的トポロジを publish
    self.publish_static_topology();

    // Startup イベントを EventStream に発火
    self.publish_startup_event(StartupMode::Client);

    Ok(())
  }

  fn shutdown(&self, _graceful: bool) -> Result<(), ClusterProviderError> {
    // 起動モードを取得してからクリア
    let mode = self.startup_mode.lock().take().unwrap_or(StartupMode::Member);

    // メンバーリストをクリア
    {
      let mut members = self.members.lock();
      members.clear();
    }

    // Shutdown イベントを EventStream に発火
    self.publish_shutdown_event(mode);

    Ok(())
  }
}
