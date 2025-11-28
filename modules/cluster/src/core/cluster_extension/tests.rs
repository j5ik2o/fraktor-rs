use alloc::{boxed::Box, string::String, vec, vec::Vec};

use fraktor_actor_rs::core::{
  event_stream::{
    EventStreamEvent, EventStreamGeneric, EventStreamSubscriber, EventStreamSubscriptionGeneric, subscriber_handle,
  },
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
  ClusterPubSub, ClusterTopology, Gossiper, IdentityLookup, IdentitySetupError, StaticClusterProvider,
};

struct StubProvider;
impl ClusterProvider for StubProvider {
  fn start_member(&mut self) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn start_client(&mut self) -> Result<(), ClusterProviderError> {
    Ok(())
  }

  fn shutdown(&mut self, _graceful: bool) -> Result<(), ClusterProviderError> {
    Ok(())
  }
}

struct StubGossiper;
impl Gossiper for StubGossiper {
  fn start(&mut self) -> Result<(), &'static str> {
    Ok(())
  }

  fn stop(&mut self) -> Result<(), &'static str> {
    Ok(())
  }
}

struct StubPubSub;
impl ClusterPubSub for StubPubSub {
  fn start(&mut self) -> Result<(), crate::core::pub_sub_error::PubSubError> {
    Ok(())
  }

  fn stop(&mut self) -> Result<(), crate::core::pub_sub_error::PubSubError> {
    Ok(())
  }
}

struct StubIdentity;
impl IdentityLookup for StubIdentity {
  fn setup_member(&mut self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }

  fn setup_client(&mut self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
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
    Box::new(StubProvider),
    ArcShared::new(StubBlockList),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
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
    Box::new(StubProvider),
    ArcShared::new(StubBlockList),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
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
    Box::new(StubProvider),
    ArcShared::new(StubBlockList),
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
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
  fn on_event(&mut self, event: &EventStreamEvent<NoStdToolbox>) {
    if let EventStreamEvent::Extension { name, payload } = event {
      if name == "cluster" {
        if let Some(cluster_event) = payload.payload().downcast_ref::<ClusterEvent>() {
          self.events.lock().push(cluster_event.clone());
        }
      }
    }
  }
}

fn subscribe_recorder(
  event_stream: &ArcShared<EventStreamGeneric<NoStdToolbox>>,
) -> (RecordingClusterEvents, EventStreamSubscriptionGeneric<NoStdToolbox>) {
  let recorder = RecordingClusterEvents::new();
  let subscriber = subscriber_handle(recorder.clone());
  let subscription = EventStreamGeneric::subscribe_arc(event_stream, &subscriber);
  (recorder, subscription)
}

/// Phase1 統合テスト: StaticClusterProvider の静的トポロジが EventStream に publish され、
/// ClusterExtension が自動的に購読して ClusterCore に適用することを検証
#[test]
fn phase1_integration_static_topology_publishes_to_event_stream_and_applies_to_core() {
  // 1. システムをセットアップ
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let event_stream = system.event_stream();

  // 2. EventStream に subscriber を登録してイベントを記録
  let (recorder, _subscription) = subscribe_recorder(&event_stream);

  // 3. StaticClusterProvider を静的トポロジで構成
  let block_list: ArcShared<dyn BlockListProvider> =
    ArcShared::new(RecordingBlockList::new(vec![String::from("blocked-node-a")]));
  let static_topology = ClusterTopology::new(1000, vec![String::from("node-b"), String::from("node-c")], vec![]);
  let provider = StaticClusterProvider::new(event_stream.clone(), block_list.clone(), "node-a")
    .with_static_topology(static_topology);

  // 4. ClusterExtension をセットアップ（metrics 有効）
  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address("node-a").with_metrics_enabled(true),
    Box::new(provider),
    block_list,
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
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
  let (recorder, _subscription) = subscribe_recorder(&event_stream);

  // 3. BlockList に複数のブロックされたノードを設定
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(RecordingBlockList::new(vec![
    String::from("blocked-1"),
    String::from("blocked-2"),
    String::from("blocked-3"),
  ]));

  // 4. StaticClusterProvider を設定
  let static_topology = ClusterTopology::new(3000, vec![String::from("node-b")], vec![]);
  let provider = StaticClusterProvider::new(event_stream.clone(), block_list.clone(), "node-a")
    .with_static_topology(static_topology);

  // 5. ClusterExtension をセットアップ
  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address("node-a").with_metrics_enabled(true),
    Box::new(provider),
    block_list,
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
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
  let (recorder, _subscription) = subscribe_recorder(&event_stream);

  // 3. ClusterExtension をセットアップ
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(StubBlockList);
  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address("node-a").with_metrics_enabled(true),
    Box::new(StubProvider),
    block_list,
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
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
    Box::new(StubProvider),
    block_list,
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
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

// ====================================================================
// Phase2 統合テスト（タスク 4.4）
// join/leave/BlockList 反映・metrics 更新・EventStream TopologyUpdated 出力を確認
// ====================================================================

/// Phase2 統合テスト: join/leave イベントが EventStream に TopologyUpdated として出力される
#[test]
fn phase2_integration_join_leave_events_produce_topology_updated() {
  // 1. システムをセットアップ
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let event_stream = system.event_stream();

  // 2. EventStream に subscriber を登録
  let (recorder, _subscription) = subscribe_recorder(&event_stream);

  // 3. ClusterExtension をセットアップ
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(StubBlockList);
  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address("node-a").with_metrics_enabled(true),
    Box::new(StubProvider),
    block_list,
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
  );
  let ext_shared = system.extended().register_extension(&ext_id);

  // 4. クラスタを開始
  ext_shared.start_member().unwrap();

  // 5. ノード join のトポロジ更新
  let join_topology = ClusterTopology::new(100, vec![String::from("node-b"), String::from("node-c")], vec![]);
  ext_shared.on_topology(&join_topology);

  // 6. ノード leave のトポロジ更新
  let leave_topology = ClusterTopology::new(200, vec![], vec![String::from("node-c")]);
  ext_shared.on_topology(&leave_topology);

  // 7. TopologyUpdated イベントが発火されたことを確認
  let topology_events = recorder.topology_updated_events();
  assert!(topology_events.len() >= 2, "At least 2 TopologyUpdated events should be fired");

  // 8. join イベントを確認
  assert!(
    topology_events.iter().any(|e| matches!(
      e,
      ClusterEvent::TopologyUpdated { joined, .. }
      if joined.contains(&String::from("node-b"))
    )),
    "TopologyUpdated should contain node-b in joined"
  );

  // 9. leave イベントを確認
  assert!(
    topology_events.iter().any(|e| matches!(
      e,
      ClusterEvent::TopologyUpdated { left, .. }
      if left.contains(&String::from("node-c"))
    )),
    "TopologyUpdated should contain node-c in left"
  );
}

/// Phase2 統合テスト: BlockList が TopologyUpdated イベントに反映される
#[test]
fn phase2_integration_blocklist_reflected_in_topology_events() {
  // 1. システムをセットアップ
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let event_stream = system.event_stream();

  // 2. EventStream に subscriber を登録
  let (recorder, _subscription) = subscribe_recorder(&event_stream);

  // 3. BlockList に複数のノードを設定
  let block_list: ArcShared<dyn BlockListProvider> =
    ArcShared::new(RecordingBlockList::new(vec![String::from("blocked-node-1"), String::from("blocked-node-2")]));

  // 4. ClusterExtension をセットアップ
  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address("node-a").with_metrics_enabled(true),
    Box::new(StubProvider),
    block_list,
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
  );
  let ext_shared = system.extended().register_extension(&ext_id);

  // 5. クラスタを開始
  ext_shared.start_member().unwrap();

  // 6. トポロジ更新を行う
  let topology = ClusterTopology::new(300, vec![String::from("node-b")], vec![]);
  ext_shared.on_topology(&topology);

  // 7. TopologyUpdated イベントに blocked が含まれていることを確認
  let topology_events = recorder.topology_updated_events();
  assert!(!topology_events.is_empty(), "TopologyUpdated should be fired");

  // 8. blocked メンバーが含まれていることを確認
  let has_blocked = topology_events.iter().any(|e| {
    if let ClusterEvent::TopologyUpdated { blocked, .. } = e {
      blocked.contains(&String::from("blocked-node-1")) && blocked.contains(&String::from("blocked-node-2"))
    } else {
      false
    }
  });
  assert!(has_blocked, "TopologyUpdated should contain blocked members");

  // 9. ClusterExtension からも blocked を取得できることを確認
  let ext_blocked = ext_shared.blocked_members();
  assert!(ext_blocked.contains(&String::from("blocked-node-1")));
  assert!(ext_blocked.contains(&String::from("blocked-node-2")));
}

/// Phase2 統合テスト: metrics が正しく更新される
#[test]
fn phase2_integration_metrics_updated_correctly_with_dynamic_topology() {
  // 1. システムをセットアップ
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();

  // 2. ClusterExtension をセットアップ
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(StubBlockList);
  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address("node-a").with_metrics_enabled(true),
    Box::new(StubProvider),
    block_list,
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
  );
  let ext_shared = system.extended().register_extension(&ext_id);

  // 3. Kind を登録して起動
  ext_shared.setup_member_kinds(vec![ActivatedKind::new("worker-kind")]).unwrap();
  ext_shared.start_member().unwrap();

  // 4. 初期メトリクス確認
  let metrics = ext_shared.metrics().unwrap();
  assert_eq!(metrics.members(), 1, "Initial members should be 1");
  assert_eq!(metrics.virtual_actors(), 2, "worker + topic = 2 virtual actors");

  // 5. 3ノード join
  let topology1 =
    ClusterTopology::new(400, vec![String::from("node-b"), String::from("node-c"), String::from("node-d")], vec![]);
  ext_shared.on_topology(&topology1);

  // 6. メトリクスが更新されたことを確認
  let metrics = ext_shared.metrics().unwrap();
  assert_eq!(metrics.members(), 4, "Members should be 1 + 3 joined = 4");

  // 7. 2ノード leave
  let topology2 = ClusterTopology::new(500, vec![], vec![String::from("node-b"), String::from("node-d")]);
  ext_shared.on_topology(&topology2);

  // 8. メトリクスが更新されたことを確認
  let metrics = ext_shared.metrics().unwrap();
  assert_eq!(metrics.members(), 2, "Members should be 4 - 2 left = 2");

  // 9. virtual_actors は変化しないことを確認
  assert_eq!(metrics.virtual_actors(), 2, "Virtual actors should remain 2");
}

/// Phase2 統合テスト: shutdown 後のメトリクスリセット
#[test]
fn phase2_integration_shutdown_resets_metrics_and_emits_event() {
  // 1. システムをセットアップ
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let event_stream = system.event_stream();

  // 2. EventStream に subscriber を登録
  let (recorder, _subscription) = subscribe_recorder(&event_stream);

  // 3. ClusterExtension をセットアップ
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(StubBlockList);
  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address("node-a").with_metrics_enabled(true),
    Box::new(StubProvider),
    block_list,
    Box::new(StubGossiper),
    Box::new(StubPubSub),
    Box::new(StubIdentity),
  );
  let ext_shared = system.extended().register_extension(&ext_id);

  // 4. Kind を登録して起動
  ext_shared.setup_member_kinds(vec![ActivatedKind::new("worker-kind")]).unwrap();
  ext_shared.start_member().unwrap();

  // 5. トポロジ更新を行う
  let topology = ClusterTopology::new(600, vec![String::from("node-b")], vec![]);
  ext_shared.on_topology(&topology);

  // 6. shutdown を呼ぶ
  ext_shared.shutdown(true).unwrap();

  // 7. Shutdown イベントが発火されたことを確認
  let events = recorder.events();
  assert!(
    events.iter().any(|e| matches!(
      e,
      ClusterEvent::Shutdown { address, mode }
      if address == "node-a" && *mode == crate::core::StartupMode::Member
    )),
    "Shutdown event should be fired"
  );

  // 8. virtual_actor_count がリセットされていることを確認
  assert_eq!(ext_shared.virtual_actor_count(), 0, "virtual_actor_count should be reset after shutdown");

  // 9. blocked_members がクリアされていることを確認
  assert!(ext_shared.blocked_members().is_empty(), "blocked_members should be cleared after shutdown");
}
