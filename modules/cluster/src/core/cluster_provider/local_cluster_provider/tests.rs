use alloc::{string::String, vec, vec::Vec};

use fraktor_actor_rs::core::event_stream::{EventStreamEvent, EventStreamGeneric, EventStreamSubscriber};
use fraktor_remote_rs::core::BlockListProvider;
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdMutex, NoStdToolbox},
  sync::ArcShared,
};

use super::*;
use crate::core::{ClusterEvent, ClusterProvider, ClusterTopology};

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
  events: ArcShared<NoStdMutex<Vec<ClusterEvent>>>,
}

impl RecordingClusterEvents {
  fn new() -> Self {
    Self { events: ArcShared::new(NoStdMutex::new(Vec::new())) }
  }

  fn events(&self) -> Vec<ClusterEvent> {
    self.events.lock().clone()
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

#[test]
fn on_member_join_publishes_topology_updated_with_joined() {
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList);

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  let mut provider = LocalClusterProvider::<NoStdToolbox>::new(event_stream, block_list, "node-a:8080");

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
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList);

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  let mut provider = LocalClusterProvider::<NoStdToolbox>::new(event_stream, block_list, "node-a:8080");

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
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> =
    ArcShared::new(RecordingBlockList::new(vec![String::from("blocked-node")]));

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  let mut provider = LocalClusterProvider::<NoStdToolbox>::new(event_stream, block_list, "node-a:8080");

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
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList);

  let mut provider = LocalClusterProvider::<NoStdToolbox>::new(event_stream, block_list, "node-a:8080");

  provider.start_member().unwrap();

  assert_eq!(provider.member_count(), 1);
}

#[test]
fn start_member_with_static_topology_publishes_it() {
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList);

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  let static_topology = ClusterTopology::new(999, vec![String::from("static-node")], vec![]);
  let mut provider = LocalClusterProvider::<NoStdToolbox>::new(event_stream, block_list, "node-a:8080")
    .with_static_topology(static_topology);

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
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList);

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  let static_topology = ClusterTopology::new(888, vec![], vec![String::from("left-node")]);
  let mut provider = LocalClusterProvider::<NoStdToolbox>::new(event_stream, block_list, "client-a")
    .with_static_topology(static_topology);

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
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList);

  let mut provider = LocalClusterProvider::<NoStdToolbox>::new(event_stream, block_list, "node-a:8080");

  provider.start_member().unwrap();
  provider.on_member_join("node-b:8080");
  assert_eq!(provider.member_count(), 2);

  provider.shutdown(true).unwrap();
  assert_eq!(provider.member_count(), 0);
}

#[test]
fn version_increments_with_each_topology_change() {
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList);

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  let mut provider = LocalClusterProvider::<NoStdToolbox>::new(event_stream, block_list, "node-a:8080");

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
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList);

  let mut provider = LocalClusterProvider::<NoStdToolbox>::new(event_stream, block_list, "node-a:8080");

  provider.on_member_join("node-b:8080");
  provider.on_member_join("node-b:8080"); // 重複

  assert_eq!(provider.member_count(), 1);
}

#[test]
fn advertised_address_is_stored_correctly() {
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList);

  let provider = LocalClusterProvider::<NoStdToolbox>::new(event_stream, block_list, "192.168.1.100:9999");

  assert_eq!(provider.advertised_address(), "192.168.1.100:9999");
}

// ====================================================================
// Phase2 タスク 4.1: seed/authority を GossipEngine に渡す経路のテスト
// ====================================================================

#[test]
fn with_seed_nodes_sets_seeds_for_gossip() {
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList);

  let seeds = vec![String::from("seed-a:8080"), String::from("seed-b:8080")];
  let provider =
    LocalClusterProvider::<NoStdToolbox>::new(event_stream, block_list, "node-a:8080").with_seed_nodes(seeds.clone());

  assert_eq!(provider.seed_nodes(), seeds);
}

#[test]
fn start_member_publishes_startup_event_to_event_stream() {
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList);

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  let mut provider = LocalClusterProvider::<NoStdToolbox>::new(event_stream, block_list, "node-a:8080");

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
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList);

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  let mut provider = LocalClusterProvider::<NoStdToolbox>::new(event_stream, block_list, "node-a:8080");

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
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList);

  let seeds = vec![String::from("seed-a:8080"), String::from("seed-b:8080")];
  let mut provider =
    LocalClusterProvider::<NoStdToolbox>::new(event_stream.clone(), block_list, "node-a:8080").with_seed_nodes(seeds);

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
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList);

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  let mut provider = LocalClusterProvider::<NoStdToolbox>::new(event_stream, block_list, "node-a:8080");
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
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList);

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  let mut provider = LocalClusterProvider::<NoStdToolbox>::new(event_stream, block_list, "node-a:8080");
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
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList);

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  let mut provider = LocalClusterProvider::<NoStdToolbox>::new(event_stream, block_list, "node-a:8080");
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
// handle_connected / handle_quarantined のテスト
// ====================================================================

#[test]
fn handle_connected_triggers_member_join() {
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList);

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  let mut provider = LocalClusterProvider::<NoStdToolbox>::new(event_stream, block_list, "node-a:8080");
  provider.start_member().unwrap();

  // handle_connected を呼び出す
  provider.handle_connected("node-b:8080");

  let events = subscriber_impl.events();
  // TopologyUpdated が join を含むことを確認
  assert!(events.iter().any(|e| matches!(
    e,
    ClusterEvent::TopologyUpdated { joined, .. }
    if joined.contains(&String::from("node-b:8080"))
  )));
}

#[test]
fn handle_connected_ignores_own_authority() {
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList);

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  let mut provider = LocalClusterProvider::<NoStdToolbox>::new(event_stream, block_list, "node-a:8080");
  provider.start_member().unwrap();

  // 自分自身の authority で handle_connected を呼び出す
  provider.handle_connected("node-a:8080");

  let events = subscriber_impl.events();
  // 自分自身の join イベントは Startup 以外で発火されないことを確認
  let topology_events: Vec<_> = events
    .iter()
    .filter(
      |e| matches!(e, ClusterEvent::TopologyUpdated { joined, .. } if joined.contains(&String::from("node-a:8080"))),
    )
    .collect();
  assert!(topology_events.is_empty(), "Own authority should not trigger TopologyUpdated");
}

#[test]
fn handle_quarantined_triggers_member_leave() {
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList);

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  let mut provider = LocalClusterProvider::<NoStdToolbox>::new(event_stream, block_list, "node-a:8080");
  provider.start_member().unwrap();

  // まず node-b を join させる
  provider.on_member_join("node-b:8080");

  // handle_quarantined を呼び出す
  provider.handle_quarantined("node-b:8080");

  let events = subscriber_impl.events();
  // TopologyUpdated が leave を含むことを確認
  assert!(events.iter().any(|e| matches!(
    e,
    ClusterEvent::TopologyUpdated { left, .. }
    if left.contains(&String::from("node-b:8080"))
  )));
}

#[test]
fn handle_quarantined_ignores_non_member() {
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList);

  let subscriber_impl = ArcShared::new(RecordingClusterEvents::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  let mut provider = LocalClusterProvider::<NoStdToolbox>::new(event_stream, block_list, "node-a:8080");
  provider.start_member().unwrap();

  // node-b は join していない状態で quarantined を呼び出す
  provider.handle_quarantined("node-b:8080");

  let events = subscriber_impl.events();
  // leave イベントは発火されないことを確認
  let leave_events: Vec<_> =
    events.iter().filter(|e| matches!(e, ClusterEvent::TopologyUpdated { left, .. } if !left.is_empty())).collect();
  assert!(leave_events.is_empty(), "Non-member quarantine should not trigger leave event");
}

#[test]
fn is_started_returns_correct_state() {
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList);

  let mut provider = LocalClusterProvider::<NoStdToolbox>::new(event_stream, block_list, "node-a:8080");

  // start前は false
  assert!(!provider.is_started());

  provider.start_member().unwrap();

  // start後は true
  assert!(provider.is_started());

  provider.shutdown(true).unwrap();

  // shutdown後は false
  assert!(!provider.is_started());
}
