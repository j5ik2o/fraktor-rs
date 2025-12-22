//! Pub/Sub integration coverage for cluster lifecycle and observability.

use fraktor_actor_rs::core::{
  event_stream::{
    EventStreamEvent, EventStreamGeneric, EventStreamSubscriber, EventStreamSubscriptionGeneric, subscriber_handle,
  },
  messaging::AnyMessageGeneric,
  serialization::{SerializationRegistryGeneric, default_serialization_setup, register_defaults},
};
use fraktor_cluster_rs::core::{
  ClusterCore, ClusterExtensionConfig, ClusterProviderShared, ClusterPubSub, ClusterPubSubImpl, ClusterPubSubShared,
  ClusterTopology, DeliverBatchRequest, DeliveryEndpoint, DeliveryEndpointSharedGeneric, DeliveryReport,
  DeliveryStatus, GossiperShared, IdentityLookupShared, KindRegistry, NoopClusterProvider, NoopGossiper,
  NoopIdentityLookup, PubSubBatch, PubSubConfig, PubSubEnvelope, PubSubError, PubSubEvent, PubSubSubscriber,
  PubSubTopic, PublishOptions, PublishRequest, SubscriberDeliveryReport,
};
use fraktor_remote_rs::core::BlockListProvider;
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdMutex, NoStdToolbox},
  sync::{ArcShared, SharedAccess},
};

#[derive(Clone)]
struct EventCollector {
  events: ArcShared<NoStdMutex<Vec<EventStreamEvent<NoStdToolbox>>>>,
}

impl EventCollector {
  fn new() -> Self {
    Self { events: ArcShared::new(NoStdMutex::new(Vec::new())) }
  }

  fn events(&self) -> Vec<EventStreamEvent<NoStdToolbox>> {
    self.events.lock().clone()
  }
}

impl EventStreamSubscriber<NoStdToolbox> for EventCollector {
  fn on_event(&mut self, event: &EventStreamEvent<NoStdToolbox>) {
    self.events.lock().push(event.clone());
  }
}

fn subscribe_recorder(
  event_stream: &ArcShared<EventStreamGeneric<NoStdToolbox>>,
) -> (EventCollector, EventStreamSubscriptionGeneric<NoStdToolbox>) {
  let subscriber = EventCollector::new();
  let handle = subscriber_handle(subscriber.clone());
  let subscription = EventStreamGeneric::subscribe_arc(event_stream, &handle);
  (subscriber, subscription)
}

fn collect_pubsub_events(events: &[EventStreamEvent<NoStdToolbox>]) -> Vec<PubSubEvent> {
  events
    .iter()
    .filter_map(|event| {
      if let EventStreamEvent::Extension { name, payload } = event
        && name == "cluster-pubsub"
      {
        return payload.payload().downcast_ref::<PubSubEvent>().cloned();
      }
      None
    })
    .collect()
}

#[derive(Clone)]
struct RecordingEndpoint {
  failed: Vec<PubSubSubscriber<NoStdToolbox>>,
}

impl RecordingEndpoint {
  fn new(failed: Vec<PubSubSubscriber<NoStdToolbox>>) -> Self {
    Self { failed }
  }
}

impl DeliveryEndpoint<NoStdToolbox> for RecordingEndpoint {
  fn deliver(
    &mut self,
    request: DeliverBatchRequest<NoStdToolbox>,
  ) -> Result<DeliveryReport<NoStdToolbox>, PubSubError> {
    let mut failed = Vec::new();
    for subscriber in request.subscribers {
      if self.failed.contains(&subscriber) {
        failed.push(SubscriberDeliveryReport { subscriber, status: DeliveryStatus::SubscriberUnreachable });
      }
    }
    let status = if failed.is_empty() { DeliveryStatus::Delivered } else { DeliveryStatus::SubscriberUnreachable };
    Ok(DeliveryReport { status, failed })
  }
}

#[derive(Clone)]
struct EmptyBlockListProvider;

impl BlockListProvider for EmptyBlockListProvider {
  fn blocked_members(&self) -> Vec<String> {
    Vec::new()
  }
}

fn make_registry() -> ArcShared<SerializationRegistryGeneric<NoStdToolbox>> {
  let setup = default_serialization_setup();
  let registry = ArcShared::new(SerializationRegistryGeneric::from_setup(&setup));
  let _ = register_defaults(&registry, |_name, _id| {});
  registry
}

fn build_pubsub(
  event_stream: ArcShared<EventStreamGeneric<NoStdToolbox>>,
  kind_registry: &KindRegistry,
  failed: Vec<PubSubSubscriber<NoStdToolbox>>,
) -> ClusterPubSubImpl<NoStdToolbox> {
  let registry = make_registry();
  let endpoint = DeliveryEndpointSharedGeneric::new(Box::new(RecordingEndpoint::new(failed)));
  ClusterPubSubImpl::new(event_stream, registry, endpoint, PubSubConfig::default(), kind_registry)
}

#[test]
fn publish_emits_delivery_and_metrics_events() {
  let event_stream: ArcShared<EventStreamGeneric<NoStdToolbox>> =
    ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let (collector, _subscription) = subscribe_recorder(&event_stream);

  let mut kind_registry = KindRegistry::new();
  kind_registry.register_all(Vec::new());

  let mut pubsub = build_pubsub(event_stream.clone(), &kind_registry, Vec::new());
  pubsub.start().expect("start");

  let topic = PubSubTopic::from("news");
  let subscriber = PubSubSubscriber::ClusterIdentity(
    fraktor_cluster_rs::core::ClusterIdentity::new("kind", "sub-1").expect("identity"),
  );
  pubsub.subscribe(&topic, subscriber).expect("subscribe");

  let batch = PubSubBatch::new(vec![PubSubEnvelope {
    serializer_id: 41,
    type_name:     String::from("dummy"),
    bytes:         vec![1],
  }]);
  let request = PublishRequest::new(topic.clone(), AnyMessageGeneric::new(batch), PublishOptions::default());
  pubsub.publish(request).expect("publish");
  pubsub.emit_metrics_snapshot();

  let events = collect_pubsub_events(&collector.events());
  assert!(
    events.iter().any(|event| matches!(event, PubSubEvent::PublishAccepted { topic: t, .. } if t.as_str() == "news"))
  );
  assert!(
    events.iter().any(|event| matches!(event, PubSubEvent::DeliverySucceeded { topic: t, .. } if t.as_str() == "news"))
  );
  assert!(events.iter().any(|event| matches!(event, PubSubEvent::MetricsSnapshot { .. })));
}

#[test]
fn topology_update_reactivates_suspended_subscribers() {
  let event_stream: ArcShared<EventStreamGeneric<NoStdToolbox>> =
    ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let (collector, _subscription) = subscribe_recorder(&event_stream);

  let mut kind_registry = KindRegistry::new();
  kind_registry.register_all(Vec::new());

  let topic = PubSubTopic::from("news");
  let subscriber = PubSubSubscriber::ClusterIdentity(
    fraktor_cluster_rs::core::ClusterIdentity::new("kind", "sub-1").expect("identity"),
  );

  let pubsub = build_pubsub(event_stream.clone(), &kind_registry, vec![subscriber.clone()]);
  let pubsub_shared = ClusterPubSubShared::new(Box::new(pubsub));

  let provider = ClusterProviderShared::new(Box::new(NoopClusterProvider::new()));
  let gossiper = GossiperShared::new(Box::new(NoopGossiper));
  let identity_lookup = IdentityLookupShared::new(Box::new(NoopIdentityLookup));
  let block_list_provider: ArcShared<dyn BlockListProvider> = ArcShared::new(EmptyBlockListProvider);
  let config = ClusterExtensionConfig::new().with_advertised_address("node-a");
  let mut core = ClusterCore::new(
    &config,
    provider,
    block_list_provider,
    event_stream.clone(),
    gossiper,
    pubsub_shared.clone(),
    kind_registry,
    identity_lookup,
  );

  core.setup_member_kinds(Vec::new()).expect("setup kinds");
  core.start_member().expect("start member");

  pubsub_shared.with_write(|pubsub| pubsub.subscribe(&topic, subscriber.clone())).expect("subscribe");

  let batch = PubSubBatch::new(vec![PubSubEnvelope {
    serializer_id: 41,
    type_name:     String::from("dummy"),
    bytes:         vec![1],
  }]);
  let request = PublishRequest::new(topic.clone(), AnyMessageGeneric::new(batch), PublishOptions::default());
  pubsub_shared.with_write(|pubsub| pubsub.publish(request)).expect("publish");

  let events_before = collector.events();
  let before_count = events_before.len();
  assert!(
    collect_pubsub_events(&events_before).iter().any(|event| matches!(event, PubSubEvent::DeliveryFailed { .. }))
  );

  let topology = ClusterTopology::new(1, vec![String::from("node-a")], Vec::new());
  core.apply_topology(&topology);

  let events_after = collector.events();
  let new_events = &events_after[before_count..];
  assert!(
    collect_pubsub_events(new_events)
      .iter()
      .any(|event| matches!(event, PubSubEvent::SubscriptionAdded { subscriber: label, .. } if label == "kind/sub-1"))
  );
}
