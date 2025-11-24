//! Sample TCP provider for Tokio-based cluster scenarios.
//!
//! This provider publishes ClusterTopology events to EventStream based on
//! membership changes. It integrates with TokioTcpTransport and provides
//! a reference implementation for TCP-based cluster providers.

extern crate std;

use std::sync::Mutex;

use fraktor_actor_rs::core::{
  event_stream::{EventStreamEvent, EventStreamGeneric},
  messaging::AnyMessageGeneric,
};
use fraktor_remote_rs::core::BlockListProvider;
use fraktor_utils_rs::{core::sync::ArcShared, std::runtime_toolbox::StdToolbox};

use crate::core::{ClusterEvent, ClusterProvider, ClusterProviderError, ClusterTopology};

#[cfg(test)]
mod tests;

/// Sample TCP provider that publishes topology events to EventStream.
///
/// This provider manages membership state and publishes `ClusterEvent::TopologyUpdated`
/// events when nodes join or leave the cluster. It serves as a reference implementation
/// for TCP-based cluster providers like etcd, zk, or automanaged providers.
pub struct SampleTcpProvider {
  event_stream:        ArcShared<EventStreamGeneric<StdToolbox>>,
  block_list_provider: ArcShared<dyn BlockListProvider>,
  advertised_address:  std::string::String,
  // 現在のメンバーリスト（join/leave イベント処理用）
  members:             Mutex<std::vec::Vec<std::string::String>>,
  // 内部バージョンカウンタ（ハッシュ生成用）
  version:             Mutex<u64>,
  // 静的トポロジ（設定されている場合、start時に publish）
  static_topology:     Mutex<Option<ClusterTopology>>,
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

  /// Returns the advertised address.
  #[must_use]
  pub fn advertised_address(&self) -> &str {
    &self.advertised_address
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
}

impl ClusterProvider for SampleTcpProvider {
  fn start_member(&self) -> Result<(), ClusterProviderError> {
    // 自分自身をメンバーリストに追加
    {
      let mut members = self.members.lock().expect("members lock");
      if !members.contains(&self.advertised_address) {
        members.push(self.advertised_address.clone());
      }
    }

    // 静的トポロジが設定されている場合は publish
    self.publish_static_topology();

    Ok(())
  }

  fn start_client(&self) -> Result<(), ClusterProviderError> {
    // クライアントモードでも静的トポロジを publish
    self.publish_static_topology();
    Ok(())
  }

  fn shutdown(&self, _graceful: bool) -> Result<(), ClusterProviderError> {
    // メンバーリストをクリア
    let mut members = self.members.lock().expect("members lock");
    members.clear();
    Ok(())
  }
}
