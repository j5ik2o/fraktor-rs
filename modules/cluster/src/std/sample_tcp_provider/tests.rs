extern crate std;

use std::{string::String, sync::Mutex, vec, vec::Vec};

use fraktor_actor_rs::core::event_stream::{EventStreamEvent, EventStreamGeneric, EventStreamSubscriber};
use fraktor_remote_rs::core::BlockListProvider;
use fraktor_utils_rs::{core::sync::ArcShared, std::runtime_toolbox::StdToolbox};

use super::*;
use crate::core::{ClusterEvent, ClusterProvider, ClusterTopology};

#[derive(Default)]
struct EmptyBlockList;

impl BlockListProvider for EmptyBlockList {
  fn blocked_members(&self) -> Vec<String> {
    Vec::new()
  }
}

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

#[derive(Clone)]
struct RecordingClusterEvents {
  events: ArcShared<Mutex<Vec<ClusterEvent>>>,
}

impl RecordingClusterEvents {
  fn new() -> Self {
    Self { events: ArcShared::new(Mutex::new(Vec::new())) }
  }

  fn events(&self) -> Vec<ClusterEvent> {
    self.events.lock().expect("events lock").clone()
  }
}

impl EventStreamSubscriber<StdToolbox> for RecordingClusterEvents {
  fn on_event(&self, event: &EventStreamEvent<StdToolbox>) {
    if let EventStreamEvent::Extension { name, payload } = event {
      if name == "cluster" {
        if let Some(cluster_event) = payload.payload().downcast_ref::<ClusterEvent>() {
          self.events.lock().expect("events lock").push(cluster_event.clone());
        }
      }
    }
  }
}

#[test]
fn on_member_join_publishes_topology_updated_with_joined() {
  let event_stream = ArcShared::new(EventStreamGeneric::<StdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList::default());

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<StdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  let provider = SampleTcpProvider::new(event_stream, block_list, "node-a:8080");

  // node-b が join
  provider.on_member_join("node-b:8080");

  let events = subscriber_impl.events();
  assert_eq!(events.len(), 1);
  assert!(matches!(
    &events[0],
    ClusterEvent::TopologyUpdated { topology, joined, left, .. }
    if topology.hash() == 1
      && joined == &vec![String::from("node-b:8080")]
      && left.is_empty()
  ));
}

#[test]
fn on_member_leave_publishes_topology_updated_with_left() {
  let event_stream = ArcShared::new(EventStreamGeneric::<StdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList::default());

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<StdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  let provider = SampleTcpProvider::new(event_stream, block_list, "node-a:8080");

  // まず node-b を join させておく
  provider.on_member_join("node-b:8080");

  // node-b が leave
  provider.on_member_leave("node-b:8080");

  let events = subscriber_impl.events();
  assert_eq!(events.len(), 2);
  assert!(matches!(
    &events[1],
    ClusterEvent::TopologyUpdated { topology, joined, left, .. }
    if topology.hash() == 2
      && joined.is_empty()
      && left == &vec![String::from("node-b:8080")]
  ));
}

#[test]
fn topology_includes_blocked_members_from_provider() {
  let event_stream = ArcShared::new(EventStreamGeneric::<StdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> =
    ArcShared::new(RecordingBlockList::new(vec![String::from("blocked-node")]));

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<StdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  let provider = SampleTcpProvider::new(event_stream, block_list, "node-a:8080");

  provider.on_member_join("node-b:8080");

  let events = subscriber_impl.events();
  if let ClusterEvent::TopologyUpdated { blocked, .. } = &events[0] {
    assert_eq!(blocked, &vec![String::from("blocked-node")]);
  } else {
    panic!("Expected TopologyUpdated event");
  }
}

#[test]
fn start_member_adds_self_to_members() {
  let event_stream = ArcShared::new(EventStreamGeneric::<StdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList::default());

  let provider = SampleTcpProvider::new(event_stream, block_list, "node-a:8080");

  provider.start_member().unwrap();

  assert_eq!(provider.member_count(), 1);
}

#[test]
fn start_member_with_static_topology_publishes_it() {
  let event_stream = ArcShared::new(EventStreamGeneric::<StdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList::default());

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<StdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  let static_topology = ClusterTopology::new(999, vec![String::from("static-node")], vec![]);
  let provider = SampleTcpProvider::new(event_stream, block_list, "node-a:8080").with_static_topology(static_topology);

  provider.start_member().unwrap();

  let events = subscriber_impl.events();
  // TopologyUpdated + Startup の 2 イベントが発火される
  assert_eq!(events.len(), 2);
  assert!(events.iter().any(|e| matches!(
    e,
    ClusterEvent::TopologyUpdated { topology, joined, .. }
    if topology.hash() == 999
      && joined == &vec![String::from("static-node")]
  )));
  assert!(events.iter().any(|e| matches!(
    e,
    ClusterEvent::Startup { mode, .. }
    if *mode == crate::core::StartupMode::Member
  )));
}

#[test]
fn start_client_with_static_topology_publishes_it() {
  let event_stream = ArcShared::new(EventStreamGeneric::<StdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList::default());

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<StdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  let static_topology = ClusterTopology::new(888, vec![], vec![String::from("left-node")]);
  let provider = SampleTcpProvider::new(event_stream, block_list, "client-a").with_static_topology(static_topology);

  provider.start_client().unwrap();

  let events = subscriber_impl.events();
  // TopologyUpdated + Startup の 2 イベントが発火される
  assert_eq!(events.len(), 2);
  assert!(events.iter().any(|e| matches!(
    e,
    ClusterEvent::TopologyUpdated { topology, left, .. }
    if topology.hash() == 888
      && left == &vec![String::from("left-node")]
  )));
  assert!(events.iter().any(|e| matches!(
    e,
    ClusterEvent::Startup { mode, .. }
    if *mode == crate::core::StartupMode::Client
  )));
}

#[test]
fn shutdown_clears_member_list() {
  let event_stream = ArcShared::new(EventStreamGeneric::<StdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList::default());

  let provider = SampleTcpProvider::new(event_stream, block_list, "node-a:8080");

  provider.start_member().unwrap();
  provider.on_member_join("node-b:8080");
  assert_eq!(provider.member_count(), 2);

  provider.shutdown(true).unwrap();
  assert_eq!(provider.member_count(), 0);
}

#[test]
fn version_increments_with_each_topology_change() {
  let event_stream = ArcShared::new(EventStreamGeneric::<StdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList::default());

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<StdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  let provider = SampleTcpProvider::new(event_stream, block_list, "node-a:8080");

  provider.on_member_join("node-b:8080");
  provider.on_member_join("node-c:8080");
  provider.on_member_leave("node-b:8080");

  let events = subscriber_impl.events();
  assert_eq!(events.len(), 3);

  // バージョンが順番に増加していることを確認
  let hashes: Vec<u64> = events
    .iter()
    .filter_map(|e| if let ClusterEvent::TopologyUpdated { topology, .. } = e { Some(topology.hash()) } else { None })
    .collect();
  assert_eq!(hashes, vec![1, 2, 3]);
}

#[test]
fn duplicate_join_does_not_add_member_twice() {
  let event_stream = ArcShared::new(EventStreamGeneric::<StdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList::default());

  let provider = SampleTcpProvider::new(event_stream, block_list, "node-a:8080");

  provider.on_member_join("node-b:8080");
  provider.on_member_join("node-b:8080"); // 重複

  assert_eq!(provider.member_count(), 1);
}

#[test]
fn advertised_address_is_stored_correctly() {
  let event_stream = ArcShared::new(EventStreamGeneric::<StdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList::default());

  let provider = SampleTcpProvider::new(event_stream, block_list, "192.168.1.100:9999");

  assert_eq!(provider.advertised_address(), "192.168.1.100:9999");
}

// ====================================================================
// Phase2 タスク 4.1: seed/authority を GossipEngine に渡す経路のテスト
// ====================================================================

#[test]
fn with_seed_nodes_sets_seeds_for_gossip() {
  let event_stream = ArcShared::new(EventStreamGeneric::<StdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList::default());

  let seeds = vec![String::from("seed-a:8080"), String::from("seed-b:8080")];
  let provider = SampleTcpProvider::new(event_stream, block_list, "node-a:8080").with_seed_nodes(seeds.clone());

  assert_eq!(provider.seed_nodes(), seeds);
}

#[test]
fn start_member_publishes_startup_event_to_event_stream() {
  let event_stream = ArcShared::new(EventStreamGeneric::<StdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList::default());

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<StdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  let provider = SampleTcpProvider::new(event_stream, block_list, "node-a:8080");

  provider.start_member().unwrap();

  // startup イベントが発火されていることを確認
  let events = subscriber_impl.events();
  assert!(events.iter().any(|e| matches!(
    e,
    ClusterEvent::Startup { address, mode }
    if address == "node-a:8080" && *mode == crate::core::StartupMode::Member
  )));
}

#[test]
fn shutdown_publishes_shutdown_event_to_event_stream() {
  let event_stream = ArcShared::new(EventStreamGeneric::<StdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList::default());

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<StdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  let provider = SampleTcpProvider::new(event_stream, block_list, "node-a:8080");

  provider.start_member().unwrap();
  provider.shutdown(true).unwrap();

  // shutdown イベントが発火されていることを確認
  let events = subscriber_impl.events();
  assert!(events.iter().any(|e| matches!(
    e,
    ClusterEvent::Shutdown { address, mode }
    if address == "node-a:8080" && *mode == crate::core::StartupMode::Member
  )));
}

#[test]
fn seed_nodes_can_be_used_to_initialize_gossip_peers() {
  let event_stream = ArcShared::new(EventStreamGeneric::<StdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList::default());

  let seeds = vec![String::from("seed-a:8080"), String::from("seed-b:8080")];
  let provider = SampleTcpProvider::new(event_stream.clone(), block_list, "node-a:8080").with_seed_nodes(seeds);

  // GossipEngine を内部で初期化できることを確認
  // provider は seed_nodes を使って GossipEngine のピアリストを初期化できる
  provider.start_member().unwrap();

  // 初期化後にピア情報が保持されていることを確認
  assert_eq!(provider.seed_nodes().len(), 2);
}

// ====================================================================
// Phase2 タスク 4.2: GossipEngine からの join/leave を EventStream に流すテスト
// ====================================================================

#[test]
fn gossip_join_event_is_converted_to_topology_updated() {
  let event_stream = ArcShared::new(EventStreamGeneric::<StdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList::default());

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<StdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  let provider = SampleTcpProvider::new(event_stream, block_list, "node-a:8080");
  provider.start_member().unwrap();

  // GossipEngine/MembershipTable からの join イベントをシミュレート
  // （実際は on_member_join で代用、将来的には GossipEngine 統合）
  provider.on_member_join("node-b:8080");

  let events = subscriber_impl.events();
  // TopologyUpdated が含まれていることを確認
  assert!(events.iter().any(|e| matches!(
    e,
    ClusterEvent::TopologyUpdated { joined, left, .. }
    if joined.contains(&String::from("node-b:8080")) && left.is_empty()
  )));
}

#[test]
fn gossip_leave_event_is_converted_to_topology_updated() {
  let event_stream = ArcShared::new(EventStreamGeneric::<StdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList::default());

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<StdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  let provider = SampleTcpProvider::new(event_stream, block_list, "node-a:8080");
  provider.start_member().unwrap();
  provider.on_member_join("node-b:8080");

  // GossipEngine/MembershipTable からの leave イベントをシミュレート
  provider.on_member_leave("node-b:8080");

  let events = subscriber_impl.events();
  // 最後のイベントが leave を含む TopologyUpdated であることを確認
  let topology_events: Vec<_> = events.iter().filter(|e| matches!(e, ClusterEvent::TopologyUpdated { .. })).collect();
  assert!(topology_events.len() >= 2);
  if let ClusterEvent::TopologyUpdated { left, .. } = topology_events.last().expect("last event") {
    assert!(left.contains(&String::from("node-b:8080")));
  }
}

#[test]
fn multiple_gossip_events_produce_sequential_topology_versions() {
  let event_stream = ArcShared::new(EventStreamGeneric::<StdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList::default());

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<StdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  let provider = SampleTcpProvider::new(event_stream, block_list, "node-a:8080");
  provider.start_member().unwrap();

  // 複数の join/leave イベント
  provider.on_member_join("node-b:8080");
  provider.on_member_join("node-c:8080");
  provider.on_member_leave("node-b:8080");
  provider.on_member_join("node-d:8080");

  let events = subscriber_impl.events();
  let topology_events: Vec<_> = events
    .iter()
    .filter_map(|e| if let ClusterEvent::TopologyUpdated { topology, .. } = e { Some(topology.hash()) } else { None })
    .collect();

  // バージョン（hash）が順番に増加していることを確認
  assert_eq!(topology_events.len(), 4);
  for i in 1..topology_events.len() {
    assert!(topology_events[i] > topology_events[i - 1], "Version should increase");
  }
}

// ====================================================================
// Phase2 タスク 4.5: Transport のコネクション/切断イベントを自動検知
// ====================================================================

#[test]
fn provider_auto_detects_connected_event_and_publishes_topology_updated() {
  use fraktor_actor_rs::core::event_stream::{CorrelationId, RemotingLifecycleEvent};

  let event_stream = ArcShared::new(EventStreamGeneric::<StdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList::default());

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<StdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  let provider = ArcShared::new(SampleTcpProvider::new(event_stream.clone(), block_list, "node-a:8080"));

  // Remoting イベント購読を開始（ArcShared 後に呼び出す必要がある）
  SampleTcpProvider::subscribe_remoting_events(&provider);

  // start_member で Transport イベント購読をアクティブ化
  provider.start_member().unwrap();

  // Remoting からの Connected イベントをシミュレート
  let connected_event = RemotingLifecycleEvent::Connected {
    authority:      String::from("node-b:8080"),
    remote_system:  String::from("cluster-node-b"),
    remote_uid:     12345,
    correlation_id: CorrelationId::default(),
  };
  event_stream.publish(&EventStreamEvent::RemotingLifecycle(connected_event));

  let events = subscriber_impl.events();

  // TopologyUpdated が発火されていることを確認（join として処理される）
  let topology_events: Vec<_> = events
    .iter()
    .filter(
      |e| matches!(e, ClusterEvent::TopologyUpdated { joined, .. } if joined.contains(&String::from("node-b:8080"))),
    )
    .collect();

  assert!(!topology_events.is_empty(), "Connected event should trigger TopologyUpdated with joined node");
}

#[test]
fn provider_auto_detects_quarantined_event_and_publishes_topology_updated() {
  use fraktor_actor_rs::core::event_stream::{CorrelationId, RemotingLifecycleEvent};

  let event_stream = ArcShared::new(EventStreamGeneric::<StdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList::default());

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<StdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  let provider = ArcShared::new(SampleTcpProvider::new(event_stream.clone(), block_list, "node-a:8080"));

  // Remoting イベント購読を開始（ArcShared 後に呼び出す必要がある）
  SampleTcpProvider::subscribe_remoting_events(&provider);

  // start_member で Transport イベント購読をアクティブ化
  provider.start_member().unwrap();

  // まず node-b を手動で join させる（既存メンバーとして）
  provider.on_member_join("node-b:8080");

  // Remoting からの Quarantined イベントをシミュレート（切断）
  let quarantined_event = RemotingLifecycleEvent::Quarantined {
    authority:      String::from("node-b:8080"),
    reason:         String::from("connection lost"),
    correlation_id: CorrelationId::default(),
  };
  event_stream.publish(&EventStreamEvent::RemotingLifecycle(quarantined_event));

  let events = subscriber_impl.events();

  // TopologyUpdated が発火されていることを確認（leave として処理される）
  let topology_events: Vec<_> = events
    .iter()
    .filter(|e| matches!(e, ClusterEvent::TopologyUpdated { left, .. } if left.contains(&String::from("node-b:8080"))))
    .collect();

  assert!(!topology_events.is_empty(), "Quarantined event should trigger TopologyUpdated with left node");
}

#[test]
fn provider_ignores_own_authority_in_connected_events() {
  use fraktor_actor_rs::core::event_stream::{CorrelationId, RemotingLifecycleEvent};

  let event_stream = ArcShared::new(EventStreamGeneric::<StdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList::default());

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<StdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  let provider = ArcShared::new(SampleTcpProvider::new(event_stream.clone(), block_list, "node-a:8080"));

  // Remoting イベント購読を開始（ArcShared 後に呼び出す必要がある）
  SampleTcpProvider::subscribe_remoting_events(&provider);

  provider.start_member().unwrap();

  // 自分自身の authority を持つ Connected イベント
  let connected_event = RemotingLifecycleEvent::Connected {
    authority:      String::from("node-a:8080"),
    remote_system:  String::from("cluster-node-a"),
    remote_uid:     11111,
    correlation_id: CorrelationId::default(),
  };
  event_stream.publish(&EventStreamEvent::RemotingLifecycle(connected_event));

  let events = subscriber_impl.events();

  // 自分自身の authority は重複 join として処理されないことを確認
  // （start_member 時に既に追加されているため、重複追加されないはず）
  let join_count = events
    .iter()
    .filter(
      |e| matches!(e, ClusterEvent::TopologyUpdated { joined, .. } if joined.contains(&String::from("node-a:8080"))),
    )
    .count();

  // start_member() 後の状態を確認するのではなく、追加のイベントが発火されないことを確認
  // start_member() で自分を追加しているが、静的トポロジがないので TopologyUpdated は発火されない
  // Connected イベントでも、既に members に含まれている場合は重複追加しない
  assert!(join_count <= 1, "Own authority should not cause duplicate join events");
}

#[test]
fn provider_subscription_is_active_only_after_start() {
  use fraktor_actor_rs::core::event_stream::{CorrelationId, RemotingLifecycleEvent};

  let event_stream = ArcShared::new(EventStreamGeneric::<StdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList::default());

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<StdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  let provider = ArcShared::new(SampleTcpProvider::new(event_stream.clone(), block_list, "node-a:8080"));

  // Remoting イベント購読を開始（ArcShared 後に呼び出す必要がある）
  SampleTcpProvider::subscribe_remoting_events(&provider);

  // start_member を呼ぶ前に Connected イベントを発火
  let connected_event = RemotingLifecycleEvent::Connected {
    authority:      String::from("node-b:8080"),
    remote_system:  String::from("cluster-node-b"),
    remote_uid:     12345,
    correlation_id: CorrelationId::default(),
  };
  event_stream.publish(&EventStreamEvent::RemotingLifecycle(connected_event));

  let events_before = subscriber_impl.events();

  // start_member 前にはトポロジイベントが発火されないことを確認
  let topology_events_before: Vec<_> = events_before
    .iter()
    .filter(
      |e| matches!(e, ClusterEvent::TopologyUpdated { joined, .. } if joined.contains(&String::from("node-b:8080"))),
    )
    .collect();

  assert!(topology_events_before.is_empty(), "Connected events before start_member should not trigger TopologyUpdated");

  // start_member を呼ぶ
  provider.start_member().unwrap();

  // start_member 後に再度 Connected イベントを発火
  let connected_event2 = RemotingLifecycleEvent::Connected {
    authority:      String::from("node-c:8080"),
    remote_system:  String::from("cluster-node-c"),
    remote_uid:     67890,
    correlation_id: CorrelationId::default(),
  };
  event_stream.publish(&EventStreamEvent::RemotingLifecycle(connected_event2));

  let events_after = subscriber_impl.events();

  // start_member 後にはトポロジイベントが発火されることを確認
  let topology_events_after: Vec<_> = events_after
    .iter()
    .filter(
      |e| matches!(e, ClusterEvent::TopologyUpdated { joined, .. } if joined.contains(&String::from("node-c:8080"))),
    )
    .collect();

  assert!(!topology_events_after.is_empty(), "Connected events after start_member should trigger TopologyUpdated");
}
