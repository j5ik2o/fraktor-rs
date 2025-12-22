#![allow(clippy::print_stdout)]

//! Distributed pub/sub demo with a pluggable delivery endpoint.

use fraktor_actor_rs::core::{
  event_stream::{EventStreamEvent, EventStreamGeneric, EventStreamSubscriber, subscriber_handle},
  messaging::AnyMessageGeneric,
  serialization::{SerializationRegistryGeneric, default_serialization_setup, register_defaults},
};
use fraktor_cluster_rs::core::{
  ClusterIdentity, ClusterPubSub, ClusterPubSubImpl, DeliverBatchRequest, DeliveryEndpoint,
  DeliveryEndpointSharedGeneric, DeliveryReport, DeliveryStatus, KindRegistry, PubSubConfig, PubSubEvent,
  PubSubSubscriber, PubSubTopic, PublishOptions, PublishRequest,
};
use fraktor_utils_rs::{core::sync::ArcShared, std::runtime_toolbox::StdToolbox};

#[derive(Clone)]
struct EventLogger;

impl EventStreamSubscriber<StdToolbox> for EventLogger {
  fn on_event(&mut self, event: &EventStreamEvent<StdToolbox>) {
    if let EventStreamEvent::Extension { name, payload } = event
      && name == "cluster-pubsub"
      && let Some(pubsub_event) = payload.payload().downcast_ref::<PubSubEvent>()
    {
      println!("[pubsub] {pubsub_event:?}");
    }
  }
}

#[derive(Default)]
struct LoggingEndpoint;

impl DeliveryEndpoint<StdToolbox> for LoggingEndpoint {
  fn deliver(
    &mut self,
    request: DeliverBatchRequest<StdToolbox>,
  ) -> Result<DeliveryReport<StdToolbox>, fraktor_cluster_rs::core::PubSubError> {
    println!("[deliver] topic={} subscribers={}", request.topic.as_str(), request.subscribers.len());
    Ok(DeliveryReport { status: DeliveryStatus::Delivered, failed: Vec::new() })
  }
}

fn main() {
  let event_stream: ArcShared<EventStreamGeneric<StdToolbox>> =
    ArcShared::new(EventStreamGeneric::<StdToolbox>::default());
  let logger = subscriber_handle(EventLogger);
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &logger);

  let setup = default_serialization_setup();
  let registry = ArcShared::new(SerializationRegistryGeneric::from_setup(&setup));
  let _ = register_defaults(&registry, |_name, _id| {});

  let mut kind_registry = KindRegistry::new();
  kind_registry.register_all(Vec::new());

  let endpoint = DeliveryEndpointSharedGeneric::new(Box::new(LoggingEndpoint::default()));
  let mut pubsub = ClusterPubSubImpl::new(event_stream, registry, endpoint, PubSubConfig::default(), &kind_registry);

  pubsub.start().expect("start");

  let topic = PubSubTopic::from("news");
  let subscriber = PubSubSubscriber::ClusterIdentity(ClusterIdentity::new("demo", "node-a").expect("identity"));
  pubsub.subscribe(&topic, subscriber).expect("subscribe");

  let request =
    PublishRequest::new(topic.clone(), AnyMessageGeneric::new(String::from("hello")), PublishOptions::default());
  let ack = pubsub.publish(request).expect("publish");
  println!("[publish] status={:?}", ack.status);
}
