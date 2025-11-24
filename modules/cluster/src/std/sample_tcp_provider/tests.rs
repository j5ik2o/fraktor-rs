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
  assert_eq!(events.len(), 1);
  assert!(matches!(
    &events[0],
    ClusterEvent::TopologyUpdated { topology, joined, .. }
    if topology.hash() == 999
      && joined == &vec![String::from("static-node")]
  ));
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
  assert_eq!(events.len(), 1);
  assert!(matches!(
    &events[0],
    ClusterEvent::TopologyUpdated { topology, left, .. }
    if topology.hash() == 888
      && left == &vec![String::from("left-node")]
  ));
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
