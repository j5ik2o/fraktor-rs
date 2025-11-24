use alloc::{string::String, vec, vec::Vec};

use fraktor_actor_rs::core::{
  event_stream::EventStreamEvent, messaging::AnyMessageGeneric, system::ActorSystemGeneric,
};
use fraktor_utils_rs::core::{runtime_toolbox::NoStdToolbox, sync::ArcShared};

use crate::core::{
  ActivatedKind, ClusterEvent, ClusterExtensionConfig, ClusterExtensionId, ClusterProvider, ClusterProviderError,
  ClusterPubSub, ClusterTopology, Gossiper, IdentityLookup, IdentitySetupError,
};

struct StubProvider;
impl ClusterProvider for StubProvider {
  fn start_member(&self) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn start_client(&self) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn shutdown(&self, _graceful: bool) -> Result<(), ClusterProviderError> {
    Ok(())
  }
}

struct StubGossiper;
impl Gossiper for StubGossiper {
  fn start(&self) -> Result<(), &'static str> {
    Ok(())
  }

  fn stop(&self) -> Result<(), &'static str> {
    Ok(())
  }
}

struct StubPubSub;
impl ClusterPubSub for StubPubSub {
  fn start(&self) -> Result<(), crate::core::pub_sub_error::PubSubError> {
    Ok(())
  }

  fn stop(&self) -> Result<(), crate::core::pub_sub_error::PubSubError> {
    Ok(())
  }
}

struct StubIdentity;
impl IdentityLookup for StubIdentity {
  fn setup_member(&self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }

  fn setup_client(&self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }
}

struct StubBlockList;
impl fraktor_remote_rs::core::BlockListProvider for StubBlockList {
  fn blocked_members(&self) -> Vec<String> {
    Vec::new()
  }
}

#[test]
fn registers_extension_and_starts_member() {
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();

  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address("fraktor://demo"),
    ArcShared::new(StubProvider),
    ArcShared::new(StubBlockList),
    ArcShared::new(StubGossiper),
    ArcShared::new(StubPubSub),
    ArcShared::new(StubIdentity),
  );

  let ext_shared = system.extended().register_extension(&ext_id);
  let result = ext_shared.start_member();
  assert!(result.is_ok());
}

#[test]
fn subscribes_to_event_stream_and_applies_topology_on_topology_updated() {
  // 1. システムとエクステンションをセットアップ
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let event_stream = system.event_stream();

  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address("fraktor://demo").with_metrics_enabled(true),
    ArcShared::new(StubProvider),
    ArcShared::new(StubBlockList),
    ArcShared::new(StubGossiper),
    ArcShared::new(StubPubSub),
    ArcShared::new(StubIdentity),
  );

  // 2. エクステンションを登録
  let ext_shared = system.extended().register_extension(&ext_id);

  // 3. エクステンションを開始（この時点で EventStream を購読するべき）
  ext_shared.start_member().unwrap();

  // 4. EventStream に TopologyUpdated イベントを publish
  let topology = ClusterTopology::new(12345, vec![String::from("node-b")], vec![]);
  let cluster_event = ClusterEvent::TopologyUpdated {
    topology: topology.clone(),
    joined:   vec![String::from("node-b")],
    left:     vec![],
    blocked:  vec![],
  };
  let payload = AnyMessageGeneric::new(cluster_event);
  let event = EventStreamEvent::Extension { name: String::from("cluster"), payload };
  event_stream.publish(&event);

  // 5. ClusterExtension が自動的に ClusterCore::on_topology を呼んだことを確認
  // metrics が更新されていればトポロジが適用されたことになる
  let metrics = ext_shared.metrics().unwrap();
  // start_member で members=1、topology で +1 joined なので members=2 を期待
  assert_eq!(metrics.members(), 2);
}

#[test]
fn ignores_topology_with_same_hash_via_event_stream() {
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let event_stream = system.event_stream();

  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address("fraktor://demo").with_metrics_enabled(true),
    ArcShared::new(StubProvider),
    ArcShared::new(StubBlockList),
    ArcShared::new(StubGossiper),
    ArcShared::new(StubPubSub),
    ArcShared::new(StubIdentity),
  );

  let ext_shared = system.extended().register_extension(&ext_id);
  ext_shared.start_member().unwrap();

  // 同じハッシュのトポロジを2回 publish
  let topology = ClusterTopology::new(99999, vec![String::from("node-x")], vec![]);
  for _ in 0..2 {
    let cluster_event = ClusterEvent::TopologyUpdated {
      topology: topology.clone(),
      joined:   vec![String::from("node-x")],
      left:     vec![],
      blocked:  vec![],
    };
    let payload = AnyMessageGeneric::new(cluster_event);
    let event = EventStreamEvent::Extension { name: String::from("cluster"), payload };
    event_stream.publish(&event);
  }

  // 重複ハッシュは抑止されるので、members は 1(initial) + 1(first topology) = 2 のまま
  let metrics = ext_shared.metrics().unwrap();
  assert_eq!(metrics.members(), 2);
}
