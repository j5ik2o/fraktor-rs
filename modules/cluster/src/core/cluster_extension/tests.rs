use alloc::{string::String, vec, vec::Vec};

use fraktor_actor_rs::core::{
  event_stream::{EventStreamEvent, EventStreamSubscriber},
  messaging::AnyMessageGeneric,
  system::ActorSystemGeneric,
};
use fraktor_remote_rs::core::BlockListProvider;
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdMutex, NoStdToolbox},
  sync::ArcShared,
};

use crate::core::{
  ActivatedKind, ClusterEvent, ClusterExtensionConfig, ClusterExtensionId, ClusterProvider, ClusterProviderError,
  ClusterPubSub, ClusterTopology, Gossiper, IdentityLookup, IdentitySetupError, InprocSampleProvider,
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

// ====================================================================
// Phase1 統合テスト: 静的トポロジ publish → EventStream → ClusterCore
// 要件1.1, 1.2, 1.4, 3.3, 5.1, 5.3 をカバー
// ====================================================================

/// BlockListProvider を実装したスタブ（blocked メンバーを返す）
struct RecordingBlockList {
  blocked: Vec<String>,
}

impl RecordingBlockList {
  fn new(blocked: Vec<String>) -> Self {
    Self { blocked }
  }
}

impl BlockListProvider for RecordingBlockList {
  fn blocked_members(&self) -> Vec<String> {
    self.blocked.clone()
  }
}

/// ClusterEvent を記録する EventStream subscriber
#[derive(Clone)]
struct RecordingClusterEvents {
  events: ArcShared<NoStdMutex<Vec<ClusterEvent>>>,
}

impl RecordingClusterEvents {
  fn new() -> Self {
    Self { events: ArcShared::new(NoStdMutex::new(Vec::new())) }
  }

  fn events(&self) -> Vec<ClusterEvent> {
    self.events.lock().clone()
  }

  fn topology_updated_events(&self) -> Vec<ClusterEvent> {
    self.events().into_iter().filter(|e| matches!(e, ClusterEvent::TopologyUpdated { .. })).collect()
  }
}

impl EventStreamSubscriber<NoStdToolbox> for RecordingClusterEvents {
  fn on_event(&self, event: &EventStreamEvent<NoStdToolbox>) {
    if let EventStreamEvent::Extension { name, payload } = event {
      if name == "cluster" {
        if let Some(cluster_event) = payload.payload().downcast_ref::<ClusterEvent>() {
          self.events.lock().push(cluster_event.clone());
        }
      }
    }
  }
}

/// Phase1 統合テスト: InprocSampleProvider の静的トポロジが EventStream に publish され、
/// ClusterExtension が自動的に購読して ClusterCore に適用することを検証
#[test]
fn phase1_integration_static_topology_publishes_to_event_stream_and_applies_to_core() {
  // 1. システムをセットアップ
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let event_stream = system.event_stream();

  // 2. EventStream に subscriber を登録してイベントを記録
  let recorder = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = recorder.clone();
  let _subscription =
    fraktor_actor_rs::core::event_stream::EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  // 3. InprocSampleProvider を静的トポロジで構成
  let block_list: ArcShared<dyn BlockListProvider> =
    ArcShared::new(RecordingBlockList::new(vec![String::from("blocked-node-a")]));
  let static_topology = ClusterTopology::new(1000, vec![String::from("node-b"), String::from("node-c")], vec![]);
  let provider =
    InprocSampleProvider::new(event_stream.clone(), block_list.clone(), "node-a").with_static_topology(static_topology);

  // 4. ClusterExtension をセットアップ（metrics 有効）
  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address("node-a").with_metrics_enabled(true),
    ArcShared::new(provider),
    block_list,
    ArcShared::new(StubGossiper),
    ArcShared::new(StubPubSub),
    ArcShared::new(StubIdentity),
  );
  let ext_shared = system.extended().register_extension(&ext_id);
  ext_shared.setup_member_kinds(vec![ActivatedKind::new("grain-kind")]).unwrap();

  // 5. クラスタを開始（start_member で provider が静的トポロジを publish）
  ext_shared.start_member().unwrap();

  // 6. EventStream に TopologyUpdated が publish されたことを確認
  let topology_events = recorder.topology_updated_events();
  assert!(!topology_events.is_empty(), "TopologyUpdated should be published to EventStream");

  // 7. ClusterCore の metrics が更新されたことを確認
  // start_member で members=1、静的トポロジで joined=2 なので members=3 を期待
  let metrics = ext_shared.metrics().unwrap();
  assert_eq!(metrics.members(), 3, "Members should include initial + joined nodes");

  // 8. blocked メンバーが反映されていることを確認
  let blocked = ext_shared.blocked_members();
  assert!(blocked.contains(&String::from("blocked-node-a")), "Blocked members should be reflected");
}

// Note: PIDキャッシュの無効化テストは cluster_core/tests.rs の
// topology_event_includes_blocked_and_updates_metrics と
// multi_node_topology_flow_updates_metrics_and_pid_cache で既にカバーされている

/// Phase1 統合テスト: blocked メンバーが TopologyUpdated イベントに含まれることを検証
#[test]
fn phase1_integration_topology_updated_includes_blocked_members() {
  // 1. システムをセットアップ
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let event_stream = system.event_stream();

  // 2. EventStream に subscriber を登録
  let recorder = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = recorder.clone();
  let _subscription =
    fraktor_actor_rs::core::event_stream::EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  // 3. BlockList に複数のブロックされたノードを設定
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(RecordingBlockList::new(vec![
    String::from("blocked-1"),
    String::from("blocked-2"),
    String::from("blocked-3"),
  ]));

  // 4. InprocSampleProvider を設定
  let static_topology = ClusterTopology::new(3000, vec![String::from("node-b")], vec![]);
  let provider =
    InprocSampleProvider::new(event_stream.clone(), block_list.clone(), "node-a").with_static_topology(static_topology);

  // 5. ClusterExtension をセットアップ
  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address("node-a").with_metrics_enabled(true),
    ArcShared::new(provider),
    block_list,
    ArcShared::new(StubGossiper),
    ArcShared::new(StubPubSub),
    ArcShared::new(StubIdentity),
  );
  let ext_shared = system.extended().register_extension(&ext_id);

  // 6. クラスタを開始
  ext_shared.start_member().unwrap();

  // 7. TopologyUpdated イベントに blocked が含まれていることを確認
  let topology_events = recorder.topology_updated_events();
  assert!(!topology_events.is_empty());

  if let ClusterEvent::TopologyUpdated { blocked, .. } = &topology_events[0] {
    assert!(blocked.contains(&String::from("blocked-1")));
    assert!(blocked.contains(&String::from("blocked-2")));
    assert!(blocked.contains(&String::from("blocked-3")));
  } else {
    panic!("Expected TopologyUpdated event");
  }

  // 8. ClusterExtension.blocked_members() からも取得できることを確認
  let ext_blocked = ext_shared.blocked_members();
  assert!(ext_blocked.contains(&String::from("blocked-1")));
  assert!(ext_blocked.contains(&String::from("blocked-2")));
  assert!(ext_blocked.contains(&String::from("blocked-3")));
}

/// Phase1 統合テスト: ハッシュが同一のトポロジは EventStream に重複 publish されないことを検証
#[test]
fn phase1_integration_duplicate_hash_topology_is_suppressed() {
  // 1. システムをセットアップ
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let event_stream = system.event_stream();

  // 2. EventStream に subscriber を登録
  let recorder = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = recorder.clone();
  let _subscription =
    fraktor_actor_rs::core::event_stream::EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  // 3. ClusterExtension をセットアップ
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(StubBlockList);
  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address("node-a").with_metrics_enabled(true),
    ArcShared::new(StubProvider),
    block_list,
    ArcShared::new(StubGossiper),
    ArcShared::new(StubPubSub),
    ArcShared::new(StubIdentity),
  );
  let ext_shared = system.extended().register_extension(&ext_id);
  ext_shared.start_member().unwrap();

  // 4. 同じハッシュのトポロジを複数回適用
  let topology = ClusterTopology::new(5000, vec![String::from("node-x")], vec![]);
  ext_shared.on_topology(&topology);
  ext_shared.on_topology(&topology); // 重複
  ext_shared.on_topology(&topology); // 重複

  // 5. TopologyUpdated は1回だけ publish されるべき
  let topology_events = recorder.topology_updated_events();
  assert_eq!(topology_events.len(), 1, "Duplicate hash topology should be suppressed");
}

/// Phase1 統合テスト: metrics 更新が正しく行われることを検証（virtual_actors 含む）
#[test]
fn phase1_integration_metrics_include_members_and_virtual_actors() {
  // 1. システムをセットアップ
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let event_stream = system.event_stream();

  // 2. ClusterExtension をセットアップ
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(StubBlockList);
  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address("node-a").with_metrics_enabled(true),
    ArcShared::new(StubProvider),
    block_list,
    ArcShared::new(StubGossiper),
    ArcShared::new(StubPubSub),
    ArcShared::new(StubIdentity),
  );
  let ext_shared = system.extended().register_extension(&ext_id);

  // 3. Kind を登録（virtual_actors が増加する）
  ext_shared.setup_member_kinds(vec![ActivatedKind::new("worker-kind"), ActivatedKind::new("analytics-kind")]).unwrap();

  // 4. クラスタを開始
  ext_shared.start_member().unwrap();

  // 5. 初期メトリクスを確認（members=1, virtual_actors=3: worker + analytics + topic）
  let metrics = ext_shared.metrics().unwrap();
  assert_eq!(metrics.members(), 1);
  assert_eq!(metrics.virtual_actors(), 3);

  // 6. トポロジを更新（2ノード join）
  let topology = ClusterTopology::new(6000, vec![String::from("node-b"), String::from("node-c")], vec![]);
  let cluster_event = ClusterEvent::TopologyUpdated {
    topology: topology.clone(),
    joined:   vec![String::from("node-b"), String::from("node-c")],
    left:     vec![],
    blocked:  vec![],
  };
  let payload = AnyMessageGeneric::new(cluster_event);
  let event = EventStreamEvent::Extension { name: String::from("cluster"), payload };
  event_stream.publish(&event);

  // 7. メトリクスが更新されたことを確認
  let metrics = ext_shared.metrics().unwrap();
  assert_eq!(metrics.members(), 3, "Members should be 1 + 2 joined");
  assert_eq!(metrics.virtual_actors(), 3, "Virtual actors should remain unchanged");
}
