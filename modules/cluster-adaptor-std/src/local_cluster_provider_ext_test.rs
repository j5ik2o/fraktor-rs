use alloc::{string::String, vec::Vec};

use fraktor_actor_core_kernel_rs::event::stream::{
  CorrelationId, EventStreamEvent, EventStreamShared, EventStreamSubscriber, EventStreamSubscriberShared,
  EventStreamSubscription, RemotingLifecycleEvent, subscriber_handle,
};
use fraktor_cluster_core_rs::{
  BlockListProvider, ClusterEvent,
  cluster_provider::{ClusterProvider, LocalClusterProvider},
};
use fraktor_utils_core_rs::sync::{ArcShared, SharedAccess, SpinSyncMutex};

use super::{subscribe_remoting_events, wrap_local_cluster_provider};

struct EmptyBlockList;

impl BlockListProvider for EmptyBlockList {
  fn blocked_members(&self) -> Vec<String> {
    Vec::new()
  }
}

#[derive(Clone)]
struct RecordingClusterEvents {
  events: ArcShared<SpinSyncMutex<Vec<ClusterEvent>>>,
}

impl RecordingClusterEvents {
  fn new() -> Self {
    Self { events: ArcShared::new(SpinSyncMutex::new(Vec::new())) }
  }

  fn events(&self) -> Vec<ClusterEvent> {
    self.events.lock().clone()
  }
}

impl EventStreamSubscriber for RecordingClusterEvents {
  fn on_event(&mut self, event: &EventStreamEvent) {
    if let EventStreamEvent::Extension { name, payload } = event
      && name == "cluster"
      && let Some(cluster_event) = payload.payload().downcast_ref::<ClusterEvent>()
    {
      self.events.lock().push(cluster_event.clone());
    }
  }
}

fn block_list() -> ArcShared<dyn BlockListProvider> {
  ArcShared::new(EmptyBlockList)
}

fn publish_connected(event_stream: &EventStreamShared, authority: &str) {
  event_stream.publish(&EventStreamEvent::RemotingLifecycle(RemotingLifecycleEvent::Connected {
    authority:      String::from(authority),
    remote_system:  String::from("remote-sys"),
    remote_uid:     7,
    correlation_id: CorrelationId::nil(),
  }));
}

fn subscribe_recorder(event_stream: &EventStreamShared) -> (RecordingClusterEvents, EventStreamSubscription) {
  let recorder = RecordingClusterEvents::new();
  let subscriber: EventStreamSubscriberShared = subscriber_handle(recorder.clone());
  let subscription = event_stream.subscribe_no_replay(&subscriber);
  (recorder, subscription)
}

#[test]
fn subscribe_remoting_events_keeps_subscription_after_helper_returns() {
  let event_stream = EventStreamShared::default();
  let provider = wrap_local_cluster_provider(LocalClusterProvider::new(
    event_stream.clone(),
    block_list(),
    "local-sys@127.0.0.1:2551",
  ));
  provider.with_write(|provider| provider.start_member().expect("provider should start"));
  let (recorder, _cluster_subscription) = subscribe_recorder(&event_stream);
  let _remoting_subscription = subscribe_remoting_events(&provider);

  publish_connected(&event_stream, "remote-sys@127.0.0.1:2552");

  assert_eq!(provider.with_read(|provider| provider.member_count()), 2);
  let events = recorder.events();
  assert!(events.iter().any(|event| matches!(
    event,
    ClusterEvent::TopologyUpdated { update }
    if update.topology.joined() == &Vec::from([String::from("remote-sys@127.0.0.1:2552")])
  )));
}

#[test]
fn dropped_remoting_subscription_stops_topology_updates() {
  let event_stream = EventStreamShared::default();
  let provider = wrap_local_cluster_provider(LocalClusterProvider::new(
    event_stream.clone(),
    block_list(),
    "local-sys@127.0.0.1:2551",
  ));
  provider.with_write(|provider| provider.start_member().expect("provider should start"));
  let (recorder, _cluster_subscription) = subscribe_recorder(&event_stream);
  let remoting_subscription = subscribe_remoting_events(&provider);
  drop(remoting_subscription);

  publish_connected(&event_stream, "remote-sys@127.0.0.1:2552");

  assert_eq!(provider.with_read(|provider| provider.member_count()), 1);
  assert!(recorder.events().is_empty());
}

#[test]
fn remoting_subscription_does_not_keep_provider_alive() {
  let event_stream = EventStreamShared::default();
  let (remoting_subscription, weak_provider) = {
    let provider = wrap_local_cluster_provider(LocalClusterProvider::new(
      event_stream.clone(),
      block_list(),
      "local-sys@127.0.0.1:2551",
    ));
    let weak_provider = provider.downgrade();
    let remoting_subscription = subscribe_remoting_events(&provider);
    (remoting_subscription, weak_provider)
  };

  assert!(weak_provider.upgrade().is_none(), "subscription must not strongly retain provider");
  publish_connected(&event_stream, "remote-sys@127.0.0.1:2552");
  drop(remoting_subscription);
}
