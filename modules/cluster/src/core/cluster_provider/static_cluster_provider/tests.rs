use alloc::{string::String, vec, vec::Vec};

use fraktor_actor_rs::core::event_stream::{
  EventStreamEvent, EventStreamGeneric, EventStreamSubscriber, EventStreamSubscriptionGeneric, subscriber_handle,
};
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
  fn on_event(&mut self, event: &EventStreamEvent<NoStdToolbox>) {
    if let EventStreamEvent::Extension { name, payload } = event
      && name == "cluster"
      && let Some(cluster_event) = payload.payload().downcast_ref::<ClusterEvent>()
    {
      self.events.lock().push(cluster_event.clone());
    }
  }
}

fn subscribe_recorder(
  event_stream: &ArcShared<EventStreamGeneric<NoStdToolbox>>,
) -> (RecordingClusterEvents, EventStreamSubscriptionGeneric<NoStdToolbox>) {
  let subscriber_impl = RecordingClusterEvents::new();
  let subscriber = subscriber_handle(subscriber_impl.clone());
  let subscription = EventStreamGeneric::subscribe_arc(event_stream, &subscriber);
  (subscriber_impl, subscription)
}

#[test]
fn start_member_publishes_static_topology_to_event_stream() {
  // EventStream を作成
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList);

  // サブスクライバを登録してイベントを記録
  let (subscriber_impl, _subscription) = subscribe_recorder(&event_stream);

  // 静的トポロジを設定した Provider を作成
  let static_topology = ClusterTopology::new(100, vec![String::from("node-b")], vec![]);
  let mut provider =
    StaticClusterProvider::new(event_stream, block_list, "node-a").with_static_topology(static_topology);

  // start_member を呼び出す
  provider.start_member().unwrap();

  // EventStream に TopologyUpdated が publish されたことを確認
  let events = subscriber_impl.events();
  assert_eq!(events.len(), 1);
  assert!(matches!(
    &events[0],
    ClusterEvent::TopologyUpdated { topology, joined, left, blocked }
    if topology.hash() == 100
      && joined == &vec![String::from("node-b")]
      && left.is_empty()
      && blocked.is_empty()
  ));
}

#[test]
fn start_client_also_publishes_static_topology() {
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList);

  let (subscriber_impl, _subscription) = subscribe_recorder(&event_stream);

  let static_topology = ClusterTopology::new(200, vec![], vec![String::from("leaving-node")]);
  let mut provider =
    StaticClusterProvider::new(event_stream, block_list, "client-a").with_static_topology(static_topology);

  provider.start_client().unwrap();

  let events = subscriber_impl.events();
  assert_eq!(events.len(), 1);
  assert!(matches!(
    &events[0],
    ClusterEvent::TopologyUpdated { topology, left, .. }
    if topology.hash() == 200 && left == &vec![String::from("leaving-node")]
  ));
}

#[test]
fn topology_includes_blocked_members_from_block_list_provider() {
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> =
    ArcShared::new(RecordingBlockList::new(vec![String::from("blocked-a"), String::from("blocked-b")]));

  let (subscriber_impl, _subscription) = subscribe_recorder(&event_stream);

  let static_topology = ClusterTopology::new(300, vec![String::from("node-x")], vec![]);
  let mut provider =
    StaticClusterProvider::new(event_stream, block_list, "node-main").with_static_topology(static_topology);

  provider.start_member().unwrap();

  let events = subscriber_impl.events();
  assert_eq!(events.len(), 1);
  if let ClusterEvent::TopologyUpdated { blocked, .. } = &events[0] {
    assert_eq!(blocked, &vec![String::from("blocked-a"), String::from("blocked-b")]);
  } else {
    panic!("Expected TopologyUpdated event");
  }
}

#[test]
fn no_topology_published_when_static_topology_not_set() {
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList);

  let (subscriber_impl, _subscription) = subscribe_recorder(&event_stream);

  // 静的トポロジを設定しない
  let mut provider = StaticClusterProvider::<NoStdToolbox>::new(event_stream, block_list, "node-empty");

  provider.start_member().unwrap();

  // イベントは publish されない
  let events = subscriber_impl.events();
  assert!(events.is_empty());
}

#[test]
fn shutdown_succeeds_without_side_effects() {
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList);

  let mut provider = StaticClusterProvider::<NoStdToolbox>::new(event_stream, block_list, "node-shutdown");

  // shutdown は常に成功
  let result = provider.shutdown(true);
  assert!(result.is_ok());

  let result = provider.shutdown(false);
  assert!(result.is_ok());
}

#[test]
fn advertised_address_is_stored_correctly() {
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let block_list: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockList);

  let provider = StaticClusterProvider::<NoStdToolbox>::new(event_stream, block_list, "127.0.0.1:8080");

  assert_eq!(provider.advertised_address(), "127.0.0.1:8080");
}
