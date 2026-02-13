use alloc::{string::String, vec::Vec};
use core::time::Duration;

use fraktor_actor_rs::core::event::stream::{
  EventStreamEvent, EventStreamShared, EventStreamSharedGeneric, EventStreamSubscriber, EventStreamSubscriptionGeneric,
  subscriber_handle,
};
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdMutex, NoStdToolbox},
  sync::ArcShared,
  time::TimerInstant,
};

use super::ClusterPubSubImpl;
use crate::core::{
  ClusterEvent, ClusterIdentity, DeliverBatchRequest, DeliveryEndpoint, DeliveryEndpointSharedGeneric, DeliveryReport,
  DeliveryStatus, KindRegistry, PubSubBatch, PubSubConfig, PubSubEnvelope, PubSubEvent, PubSubSubscriber, PubSubTopic,
  PublishAck, PublishOptions, PublishRequest, SubscriberDeliveryReport, TopologyUpdate, cluster_pub_sub::ClusterPubSub,
};

/// EventStream イベントを収集するテスト用 subscriber
#[derive(Clone)]
struct TestSubscriber {
  events: ArcShared<NoStdMutex<Vec<EventStreamEvent<NoStdToolbox>>>>,
}

impl TestSubscriber {
  fn new() -> Self {
    Self { events: ArcShared::new(NoStdMutex::new(Vec::new())) }
  }

  fn events(&self) -> Vec<EventStreamEvent<NoStdToolbox>> {
    self.events.lock().clone()
  }
}

impl EventStreamSubscriber<NoStdToolbox> for TestSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent<NoStdToolbox>) {
    self.events.lock().push(event.clone());
  }
}

fn subscribe_recorder(
  event_stream: &EventStreamSharedGeneric<NoStdToolbox>,
) -> (TestSubscriber, EventStreamSubscriptionGeneric<NoStdToolbox>) {
  let subscriber = TestSubscriber::new();
  let handle = subscriber_handle(subscriber.clone());
  let subscription = event_stream.subscribe(&handle);
  (subscriber, subscription)
}

fn extract_cluster_events(events: &[EventStreamEvent<NoStdToolbox>]) -> Vec<ClusterEvent> {
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

fn extract_pub_sub_events(events: &[EventStreamEvent<NoStdToolbox>]) -> Vec<PubSubEvent> {
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
  failed: Vec<PubSubSubscriber<NoStdToolbox>>,
}

impl StubEndpoint {
  fn new(failed: Vec<PubSubSubscriber<NoStdToolbox>>) -> Self {
    Self { failed }
  }
}

impl DeliveryEndpoint<NoStdToolbox> for StubEndpoint {
  fn deliver(
    &mut self,
    request: DeliverBatchRequest<NoStdToolbox>,
  ) -> Result<DeliveryReport<NoStdToolbox>, crate::core::PubSubError> {
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
  event_stream: EventStreamSharedGeneric<NoStdToolbox>,
  registry: &KindRegistry,
  failed: Vec<PubSubSubscriber<NoStdToolbox>>,
) -> ClusterPubSubImpl<NoStdToolbox> {
  let setup = fraktor_actor_rs::core::serialization::default_serialization_setup();
  let serialization_registry = ArcShared::new(
    fraktor_actor_rs::core::serialization::serialization_registry::SerializationRegistryGeneric::from_setup(&setup),
  );
  let endpoint = DeliveryEndpointSharedGeneric::new(Box::new(StubEndpoint::new(failed)));
  ClusterPubSubImpl::new(event_stream, serialization_registry, endpoint, PubSubConfig::default(), registry)
}

#[test]
fn starts_when_topic_kind_is_registered() {
  let mut registry = KindRegistry::new();
  registry.register_all(Vec::new());

  let event_stream: EventStreamSharedGeneric<NoStdToolbox> = EventStreamShared::default();
  let (_subscriber, _subscription) = subscribe_recorder(&event_stream);

  let mut pubsub = make_pubsub(event_stream, &registry, Vec::new());
  let result = pubsub.start();
  assert!(result.is_ok(), "start should succeed when TopicActorKind is registered");
}

#[test]
fn fails_and_fires_event_when_topic_kind_missing() {
  let registry = KindRegistry::new();
  let event_stream: EventStreamSharedGeneric<NoStdToolbox> = EventStreamShared::default();
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

  let event_stream: EventStreamSharedGeneric<NoStdToolbox> = EventStreamShared::default();
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
  let request = PublishRequest::new(
    topic.clone(),
    fraktor_actor_rs::core::messaging::AnyMessageGeneric::new(batch),
    PublishOptions::default(),
  );
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
  let event_stream: EventStreamSharedGeneric<NoStdToolbox> = EventStreamShared::default();

  let mut pubsub = make_pubsub(event_stream, &registry, Vec::new());
  pubsub.start().expect("start");

  let topic = PubSubTopic::from("news");
  let batch = PubSubBatch::new(vec![PubSubEnvelope {
    serializer_id: 41,
    type_name:     String::from("dummy"),
    bytes:         vec![1],
  }]);
  let request = PublishRequest::new(
    topic.clone(),
    fraktor_actor_rs::core::messaging::AnyMessageGeneric::new(batch),
    PublishOptions::default(),
  );
  let ack = pubsub.publish(request).expect("publish");
  assert_eq!(ack.status, crate::core::PublishStatus::Rejected);
}

#[test]
fn topology_update_reactivates_suspended_subscribers() {
  let mut registry = KindRegistry::new();
  registry.register_all(Vec::new());

  let event_stream: EventStreamSharedGeneric<NoStdToolbox> = EventStreamShared::default();
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
  let request = PublishRequest::new(
    topic.clone(),
    fraktor_actor_rs::core::messaging::AnyMessageGeneric::new(batch),
    PublishOptions::default(),
  );
  let _ = pubsub.publish(request).expect("publish");

  let topology = crate::core::ClusterTopology::new(1, vec![String::from("node-a")], Vec::new(), Vec::new());
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
