use alloc::{string::String, vec::Vec};
use core::time::Duration;

use fraktor_actor_core_rs::core::kernel::{
  actor::messaging::AnyMessage,
  event::stream::{
    EventStreamEvent, EventStreamShared, EventStreamSubscriber, EventStreamSubscriberShared, EventStreamSubscription,
    subscriber_handle_with_shared_factory,
  },
  serialization::{default_serialization_setup, serialization_registry::SerializationRegistry},
  system::shared_factory::BuiltinSpinSharedFactory,
};
use fraktor_utils_core_rs::core::{
  sync::{ArcShared, SpinSyncMutex},
  time::TimerInstant,
};

use super::ClusterPubSubImpl;
use crate::core::{
  ClusterEvent, ClusterTopology, TopologyUpdate,
  grain::KindRegistry,
  identity::ClusterIdentity,
  pub_sub::{
    DeliverBatchRequest, DeliveryEndpoint, DeliveryEndpointShared, DeliveryReport, DeliveryStatus, PubSubBatch,
    PubSubConfig, PubSubEnvelope, PubSubError, PubSubEvent, PubSubSubscriber, PubSubTopic, PublishAck, PublishOptions,
    PublishRequest, SubscriberDeliveryReport, cluster_pub_sub::ClusterPubSub,
  },
};

/// EventStream イベントを収集するテスト用 subscriber
#[derive(Clone)]
struct TestSubscriber {
  events: ArcShared<SpinSyncMutex<Vec<EventStreamEvent>>>,
}

impl TestSubscriber {
  fn new() -> Self {
    Self { events: ArcShared::new(SpinSyncMutex::new(Vec::new())) }
  }

  fn events(&self) -> Vec<EventStreamEvent> {
    self.events.lock().clone()
  }
}

impl EventStreamSubscriber for TestSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
    self.events.lock().push(event.clone());
  }
}

fn subscribe_recorder(event_stream: &EventStreamShared) -> (TestSubscriber, EventStreamSubscription) {
  let subscriber = TestSubscriber::new();
  let handle = test_subscriber_handle(subscriber.clone());
  let subscription = event_stream.subscribe(&handle);
  (subscriber, subscription)
}

fn test_subscriber_handle(subscriber: impl EventStreamSubscriber) -> EventStreamSubscriberShared {
  let provider = ArcShared::new(BuiltinSpinSharedFactory::new());
  let lock_provider: ArcShared<
    dyn fraktor_actor_core_rs::core::kernel::event::stream::EventStreamSubscriberSharedFactory,
  > = provider;
  subscriber_handle_with_shared_factory(&lock_provider, subscriber)
}

fn extract_cluster_events(events: &[EventStreamEvent]) -> Vec<ClusterEvent> {
  events
    .iter()
    .filter_map(|e| {
      if let EventStreamEvent::Extension { name, payload } = e
        && name == "cluster"
      {
        return payload.payload().downcast_ref::<ClusterEvent>().cloned();
      }
      None
    })
    .collect()
}

fn extract_pub_sub_events(events: &[EventStreamEvent]) -> Vec<PubSubEvent> {
  events
    .iter()
    .filter_map(|e| {
      if let EventStreamEvent::Extension { name, payload } = e
        && name == "cluster-pubsub"
      {
        return payload.payload().downcast_ref::<PubSubEvent>().cloned();
      }
      None
    })
    .collect()
}

#[derive(Clone)]
struct StubEndpoint {
  failed: Vec<PubSubSubscriber>,
}

impl StubEndpoint {
  fn new(failed: Vec<PubSubSubscriber>) -> Self {
    Self { failed }
  }
}

impl DeliveryEndpoint for StubEndpoint {
  fn deliver(&mut self, request: DeliverBatchRequest) -> Result<DeliveryReport, PubSubError> {
    let failed = request
      .subscribers
      .into_iter()
      .filter(|subscriber| self.failed.contains(subscriber))
      .map(|subscriber| SubscriberDeliveryReport { subscriber, status: DeliveryStatus::SubscriberUnreachable })
      .collect();
    Ok(DeliveryReport { status: DeliveryStatus::Delivered, failed })
  }
}

fn make_pubsub(
  event_stream: EventStreamShared,
  registry: &KindRegistry,
  failed: Vec<PubSubSubscriber>,
) -> ClusterPubSubImpl {
  let setup = default_serialization_setup();
  let serialization_registry = ArcShared::new(SerializationRegistry::from_setup(&setup));
  let endpoint = DeliveryEndpointShared::new(Box::new(StubEndpoint::new(failed)));
  ClusterPubSubImpl::new(event_stream, serialization_registry, endpoint, PubSubConfig::default(), registry)
}

#[test]
fn starts_when_topic_kind_is_registered() {
  let mut registry = KindRegistry::new();
  registry.register_all(Vec::new());

  let event_stream: EventStreamShared = EventStreamShared::default();
  let (_subscriber, _subscription) = subscribe_recorder(&event_stream);

  let mut pubsub = make_pubsub(event_stream, &registry, Vec::new());
  let result = pubsub.start();
  assert!(result.is_ok(), "start should succeed when TopicActorKind is registered");
}

#[test]
fn fails_and_fires_event_when_topic_kind_missing() {
  let registry = KindRegistry::new();
  let event_stream: EventStreamShared = EventStreamShared::default();
  let (subscriber, _subscription) = subscribe_recorder(&event_stream);

  let mut pubsub = make_pubsub(event_stream, &registry, Vec::new());
  let result = pubsub.start();
  assert!(result.is_err(), "start should fail when TopicActorKind is not registered");

  let collected = subscriber.events();
  let cluster_events = extract_cluster_events(&collected);
  assert!(
    cluster_events
      .iter()
      .any(|e| matches!(e, ClusterEvent::StartupFailed { reason, .. } if reason.contains("TopicActorKind"))),
    "should emit StartupFailed event with reason containing TopicActorKind"
  );
}

#[test]
fn publish_accepts_and_emits_events() {
  let mut registry = KindRegistry::new();
  registry.register_all(Vec::new());

  let event_stream: EventStreamShared = EventStreamShared::default();
  let (subscriber, _subscription) = subscribe_recorder(&event_stream);

  let mut pubsub = make_pubsub(event_stream, &registry, Vec::new());
  pubsub.start().expect("start");

  let topic = PubSubTopic::from("news");
  let subscriber_id = PubSubSubscriber::ClusterIdentity(ClusterIdentity::new("kind", "sub-1").expect("identity"));
  pubsub.subscribe(&topic, subscriber_id.clone()).expect("subscribe");

  let batch = PubSubBatch::new(vec![PubSubEnvelope {
    serializer_id: 41,
    type_name:     String::from("dummy"),
    bytes:         vec![1],
  }]);
  let request = PublishRequest::new(topic.clone(), AnyMessage::new(batch), PublishOptions::default());
  let ack = pubsub.publish(request).expect("publish");
  assert_eq!(ack, PublishAck::accepted());

  let events = extract_pub_sub_events(&subscriber.events());
  assert!(
    events.iter().any(|event| matches!(event, PubSubEvent::PublishAccepted { topic: t, .. } if t.as_str() == "news"))
  );
  assert!(events.iter().any(|event| matches!(event, PubSubEvent::DeliverySucceeded { topic: t, subscriber } if t.as_str() == "news" && subscriber == "kind/sub-1")));
}

#[test]
fn publish_rejects_when_no_subscribers() {
  let mut registry = KindRegistry::new();
  registry.register_all(Vec::new());
  let event_stream: EventStreamShared = EventStreamShared::default();

  let mut pubsub = make_pubsub(event_stream, &registry, Vec::new());
  pubsub.start().expect("start");

  let topic = PubSubTopic::from("news");
  let batch = PubSubBatch::new(vec![PubSubEnvelope {
    serializer_id: 41,
    type_name:     String::from("dummy"),
    bytes:         vec![1],
  }]);
  let request = PublishRequest::new(topic.clone(), AnyMessage::new(batch), PublishOptions::default());
  let ack = pubsub.publish(request).expect("publish");
  assert_eq!(ack.status, crate::core::pub_sub::PublishStatus::Rejected);
}

#[test]
fn topology_update_reactivates_suspended_subscribers() {
  let mut registry = KindRegistry::new();
  registry.register_all(Vec::new());

  let event_stream: EventStreamShared = EventStreamShared::default();
  let (subscriber, _subscription) = subscribe_recorder(&event_stream);

  let topic = PubSubTopic::from("news");
  let subscriber_id = PubSubSubscriber::ClusterIdentity(ClusterIdentity::new("kind", "sub-1").expect("identity"));
  let mut pubsub = make_pubsub(event_stream.clone(), &registry, vec![subscriber_id.clone()]);
  pubsub.start().expect("start");
  pubsub.subscribe(&topic, subscriber_id.clone()).expect("subscribe");

  let batch = PubSubBatch::new(vec![PubSubEnvelope {
    serializer_id: 41,
    type_name:     String::from("dummy"),
    bytes:         vec![1],
  }]);
  let request = PublishRequest::new(topic.clone(), AnyMessage::new(batch), PublishOptions::default());
  let _ = pubsub.publish(request).expect("publish");

  let topology = ClusterTopology::new(1, vec![String::from("node-a")], Vec::new(), Vec::new());
  let update = TopologyUpdate::new(
    topology,
    vec![String::from("node-a")],
    vec![String::from("node-a")],
    Vec::new(),
    Vec::new(),
    Vec::new(),
    TimerInstant::from_ticks(2, Duration::from_secs(1)),
  );
  pubsub.on_topology(&update);

  let events = extract_pub_sub_events(&subscriber.events());
  assert!(events.iter().any(|event| matches!(event, PubSubEvent::DeliveryFailed { topic: t, subscriber, .. }
    if t.as_str() == "news" && subscriber == "kind/sub-1")));
  assert!(events.iter().any(|event| matches!(event, PubSubEvent::SubscriptionAdded { topic: t, subscriber }
    if t.as_str() == "news" && subscriber == "kind/sub-1")));
}
