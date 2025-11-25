//! Sample TCP provider for Tokio-based cluster scenarios.
//!
//! This provider publishes ClusterTopology events to EventStream based on
//! membership changes. It integrates with TokioTcpTransport and provides
//! a reference implementation for TCP-based cluster providers.
//!
//! Phase2 Task 4.5: Transport のコネクション/切断イベントを自動検知し、
//! TopologyUpdated を自動的に publish する機能を実装。

extern crate std;

use std::sync::{Arc, Mutex, Weak};

use fraktor_actor_rs::core::{
  event_stream::{
    EventStreamEvent, EventStreamGeneric, EventStreamSubscriber, EventStreamSubscriptionGeneric, RemotingLifecycleEvent,
  },
  messaging::AnyMessageGeneric,
};
use fraktor_remote_rs::core::BlockListProvider;
use fraktor_utils_rs::{core::sync::ArcShared, std::runtime_toolbox::StdToolbox};

use crate::core::{ClusterEvent, ClusterProvider, ClusterProviderError, ClusterTopology, StartupMode};

#[cfg(test)]
mod tests;

/// Sample TCP provider that publishes topology events to EventStream.
///
/// This provider manages membership state and publishes `ClusterEvent::TopologyUpdated`
/// events when nodes join or leave the cluster. It serves as a reference implementation
/// for TCP-based cluster providers like etcd, zk, or automanaged providers.
///
/// Phase2 では seed_nodes を使って GossipEngine のピアリストを初期化し、
/// 起動/停止イベントを EventStream に発火します。
///
/// Task 4.5: Transport の `RemotingLifecycleEvent::Connected` と `Quarantined`
/// イベントを自動検知し、`TopologyUpdated` を publish します。
pub struct SampleTcpProvider {
  event_stream:          ArcShared<EventStreamGeneric<StdToolbox>>,
  block_list_provider:   ArcShared<dyn BlockListProvider>,
  advertised_address:    std::string::String,
  // 現在のメンバーリスト（join/leave イベント処理用）
  members:               Mutex<std::vec::Vec<std::string::String>>,
  // 内部バージョンカウンタ（ハッシュ生成用）
  version:               Mutex<u64>,
  // 静的トポロジ（設定されている場合、start時に publish）
  static_topology:       Mutex<Option<ClusterTopology>>,
  // GossipEngine 用の seed ノードリスト（Phase2）
  seed_nodes:            Mutex<std::vec::Vec<std::string::String>>,
  // 起動モード（Member/Client）を追跡
  startup_mode:          Mutex<Option<StartupMode>>,
  // Remoting イベント購読（Task 4.5: Transport イベント自動検知）
  remoting_subscription: Mutex<Option<EventStreamSubscriptionGeneric<StdToolbox>>>,
}

impl SampleTcpProvider {
  /// Creates a new sample TCP provider.
  #[must_use]
  pub fn new(
    event_stream: ArcShared<EventStreamGeneric<StdToolbox>>,
    block_list_provider: ArcShared<dyn BlockListProvider>,
    advertised_address: impl Into<std::string::String>,
  ) -> Self {
    Self {
      event_stream,
      block_list_provider,
      advertised_address: advertised_address.into(),
      members: Mutex::new(std::vec::Vec::new()),
      version: Mutex::new(0),
      static_topology: Mutex::new(None),
      seed_nodes: Mutex::new(std::vec::Vec::new()),
      startup_mode: Mutex::new(None),
      remoting_subscription: Mutex::new(None),
    }
  }

  /// Sets a static topology to be published on startup.
  ///
  /// This is useful for testing or scenarios where topology is predetermined.
  #[must_use]
  pub fn with_static_topology(self, topology: ClusterTopology) -> Self {
    *self.static_topology.lock().expect("static_topology lock") = Some(topology);
    self
  }

  /// Sets the seed nodes for GossipEngine initialization.
  ///
  /// These nodes will be used as initial peers when the provider starts.
  /// In Phase2, this enables GossipEngine to establish connections with known peers.
  #[must_use]
  pub fn with_seed_nodes(self, seeds: std::vec::Vec<std::string::String>) -> Self {
    *self.seed_nodes.lock().expect("seed_nodes lock") = seeds;
    self
  }

  /// Returns the advertised address.
  #[must_use]
  pub fn advertised_address(&self) -> &str {
    &self.advertised_address
  }

  /// Returns the configured seed nodes.
  #[must_use]
  pub fn seed_nodes(&self) -> std::vec::Vec<std::string::String> {
    self.seed_nodes.lock().expect("seed_nodes lock").clone()
  }

  /// Notifies the provider that a node has joined the cluster.
  ///
  /// This will publish a `ClusterEvent::TopologyUpdated` with the joined node
  /// in the `joined` list.
  pub fn on_member_join(&self, authority: impl Into<std::string::String>) {
    let authority = authority.into();
    let mut members = self.members.lock().expect("members lock");
    if !members.contains(&authority) {
      members.push(authority.clone());
    }
    drop(members);

    let version = self.next_version();
    self.publish_topology(version, std::vec![authority], std::vec![]);
  }

  /// Notifies the provider that a node has left the cluster.
  ///
  /// This will publish a `ClusterEvent::TopologyUpdated` with the left node
  /// in the `left` list.
  pub fn on_member_leave(&self, authority: impl Into<std::string::String>) {
    let authority = authority.into();
    let mut members = self.members.lock().expect("members lock");
    members.retain(|m| m != &authority);
    drop(members);

    let version = self.next_version();
    self.publish_topology(version, std::vec![], std::vec![authority]);
  }

  /// Returns the current member count.
  #[must_use]
  pub fn member_count(&self) -> usize {
    self.members.lock().expect("members lock").len()
  }

  fn next_version(&self) -> u64 {
    let mut version = self.version.lock().expect("version lock");
    *version += 1;
    *version
  }

  fn publish_topology(
    &self,
    version: u64,
    joined: std::vec::Vec<std::string::String>,
    left: std::vec::Vec<std::string::String>,
  ) {
    let blocked = self.block_list_provider.blocked_members();
    let topology = ClusterTopology::new(version, joined.clone(), left.clone());
    let event = ClusterEvent::TopologyUpdated { topology, joined, left, blocked };
    let payload = AnyMessageGeneric::new(event);
    let extension_event = EventStreamEvent::Extension { name: std::string::String::from("cluster"), payload };
    self.event_stream.publish(&extension_event);
  }

  fn publish_static_topology(&self) {
    let static_topology = self.static_topology.lock().expect("static_topology lock");
    if let Some(topology) = static_topology.as_ref() {
      let blocked = self.block_list_provider.blocked_members();
      let event = ClusterEvent::TopologyUpdated {
        topology: topology.clone(),
        joined: topology.joined().clone(),
        left: topology.left().clone(),
        blocked,
      };
      let payload = AnyMessageGeneric::new(event);
      let extension_event = EventStreamEvent::Extension { name: std::string::String::from("cluster"), payload };
      self.event_stream.publish(&extension_event);
    }
  }

  fn publish_startup_event(&self, mode: StartupMode) {
    let event = ClusterEvent::Startup { address: self.advertised_address.clone(), mode };
    let payload = AnyMessageGeneric::new(event);
    let extension_event = EventStreamEvent::Extension { name: std::string::String::from("cluster"), payload };
    self.event_stream.publish(&extension_event);
  }

  fn publish_shutdown_event(&self, mode: StartupMode) {
    let event = ClusterEvent::Shutdown { address: self.advertised_address.clone(), mode };
    let payload = AnyMessageGeneric::new(event);
    let extension_event = EventStreamEvent::Extension { name: std::string::String::from("cluster"), payload };
    self.event_stream.publish(&extension_event);
  }
}

impl SampleTcpProvider {
  /// Subscribes to remoting lifecycle events for automatic topology updates.
  ///
  /// This should be called after the provider is wrapped in `ArcShared`.
  /// The provider will automatically detect `Connected` and `Quarantined` events
  /// from the transport layer and publish corresponding `TopologyUpdated` events.
  pub fn subscribe_remoting_events(provider: &ArcShared<Self>) {
    // ArcShared から内部の Arc を取得して Weak を作成
    let arc_inner: Arc<Self> = provider.clone().___into_arc();
    let weak_provider = Arc::downgrade(&arc_inner);
    let subscriber: ArcShared<dyn EventStreamSubscriber<StdToolbox>> =
      ArcShared::new(RemotingEventHandler::new(weak_provider));
    let subscription = EventStreamGeneric::subscribe_arc(&provider.event_stream, &subscriber);
    *provider.remoting_subscription.lock().expect("remoting_subscription lock") = Some(subscription);
  }

  /// Handles a connected event from remoting, triggering a member join.
  fn handle_connected(&self, authority: &str) {
    // 自分自身の authority は無視（既に members に含まれているはず）
    if authority == self.advertised_address {
      return;
    }
    // メンバーリストに追加されていない場合のみ join イベントを発火
    let should_join = {
      let members = self.members.lock().expect("members lock");
      !members.contains(&std::string::String::from(authority))
    };
    if should_join {
      self.on_member_join(authority);
    }
  }

  /// Handles a quarantined event from remoting, triggering a member leave.
  fn handle_quarantined(&self, authority: &str) {
    // 自分自身の authority は無視
    if authority == self.advertised_address {
      return;
    }
    // メンバーリストに含まれている場合のみ leave イベントを発火
    let should_leave = {
      let members = self.members.lock().expect("members lock");
      members.contains(&std::string::String::from(authority))
    };
    if should_leave {
      self.on_member_leave(authority);
    }
  }

  /// Returns whether the provider has been started.
  fn is_started(&self) -> bool {
    self.startup_mode.lock().expect("startup_mode lock").is_some()
  }
}

/// Internal handler for remoting lifecycle events.
///
/// This subscriber listens to `RemotingLifecycleEvent::Connected` and
/// `RemotingLifecycleEvent::Quarantined` events and delegates to
/// the provider's join/leave logic.
struct RemotingEventHandler {
  provider: Weak<SampleTcpProvider>,
}

impl RemotingEventHandler {
  fn new(provider: Weak<SampleTcpProvider>) -> Self {
    Self { provider }
  }
}

impl EventStreamSubscriber<StdToolbox> for RemotingEventHandler {
  fn on_event(&self, event: &EventStreamEvent<StdToolbox>) {
    // 弱参照から provider を取得
    let Some(provider) = self.provider.upgrade() else {
      return;
    };

    // 起動前はイベントを無視
    if !provider.is_started() {
      return;
    }

    // RemotingLifecycle イベントのみ処理
    if let EventStreamEvent::RemotingLifecycle(lifecycle_event) = event {
      match lifecycle_event {
        | RemotingLifecycleEvent::Connected { authority, .. } => {
          provider.handle_connected(authority);
        },
        | RemotingLifecycleEvent::Quarantined { authority, .. } => {
          provider.handle_quarantined(authority);
        },
        | _ => {
          // その他のライフサイクルイベントは現時点では無視
        },
      }
    }
  }
}

impl ClusterProvider for SampleTcpProvider {
  fn start_member(&self) -> Result<(), ClusterProviderError> {
    // 起動モードを設定
    *self.startup_mode.lock().expect("startup_mode lock") = Some(StartupMode::Member);

    // 自分自身をメンバーリストに追加
    {
      let mut members = self.members.lock().expect("members lock");
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
    *self.startup_mode.lock().expect("startup_mode lock") = Some(StartupMode::Client);

    // クライアントモードでも静的トポロジを publish
    self.publish_static_topology();

    // Startup イベントを EventStream に発火
    self.publish_startup_event(StartupMode::Client);

    Ok(())
  }

  fn shutdown(&self, _graceful: bool) -> Result<(), ClusterProviderError> {
    // Remoting イベントの購読を解除
    let _subscription = self.remoting_subscription.lock().expect("remoting_subscription lock").take();

    // 起動モードを取得してからクリア
    let mode = self.startup_mode.lock().expect("startup_mode lock").take().unwrap_or(StartupMode::Member);

    // メンバーリストをクリア
    {
      let mut members = self.members.lock().expect("members lock");
      members.clear();
    }

    // Shutdown イベントを EventStream に発火
    self.publish_shutdown_event(mode);

    Ok(())
  }
}
