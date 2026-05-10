use alloc::vec::Vec;
use core::time::Duration;

use fraktor_actor_adaptor_std_rs::system::new_noop_actor_system;
use fraktor_actor_core_kernel_rs::{
  actor::messaging::AnyMessage,
  serialization::{
    builtin::register_defaults, default_serialization_setup, serialization_registry::SerializationRegistry,
  },
};
use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};

use super::BatchingProducer;
use crate::{
  TopologyUpdate,
  pub_sub::{
    BatchingProducerConfig, ClusterPubSubShared, PubSubBatch, PubSubError, PubSubPublisher, PubSubSubscriber,
    PubSubTopic, PublishAck, PublishRejectReason, PublishRequest, cluster_pub_sub::ClusterPubSub,
  },
};

#[derive(Clone)]
struct RecordingPubSub {
  batches: ArcShared<SpinSyncMutex<Vec<usize>>>,
}

impl RecordingPubSub {
  fn new() -> Self {
    Self { batches: ArcShared::new(SpinSyncMutex::new(Vec::new())) }
  }

  fn batches(&self) -> Vec<usize> {
    self.batches.lock().clone()
  }
}

impl ClusterPubSub for RecordingPubSub {
  fn start(&mut self) -> Result<(), PubSubError> {
    Ok(())
  }

  fn stop(&mut self) -> Result<(), PubSubError> {
    Ok(())
  }

  fn subscribe(&mut self, _topic: &PubSubTopic, _subscriber: PubSubSubscriber) -> Result<(), PubSubError> {
    Ok(())
  }

  fn unsubscribe(&mut self, _topic: &PubSubTopic, _subscriber: PubSubSubscriber) -> Result<(), PubSubError> {
    Ok(())
  }

  fn publish(&mut self, request: PublishRequest) -> Result<PublishAck, PubSubError> {
    if let Some(batch) = request.payload.payload().downcast_ref::<PubSubBatch>() {
      self.batches.lock().push(batch.envelopes.len());
    }
    Ok(PublishAck::accepted())
  }

  fn on_topology(&mut self, _update: &TopologyUpdate) {}
}

fn make_registry() -> ArcShared<SerializationRegistry> {
  let setup = default_serialization_setup();
  let registry = ArcShared::new(SerializationRegistry::from_setup(&setup));
  let _ = register_defaults(&registry, |_name, _id| {});
  registry
}

#[test]
fn flushes_when_batch_size_reached() {
  let registry = make_registry();
  let pubsub = RecordingPubSub::new();
  let shared = ClusterPubSubShared::new(Box::new(pubsub.clone()));
  let publisher = PubSubPublisher::new(shared, registry);

  let system = new_noop_actor_system();
  let scheduler = system.state().scheduler();

  let config = BatchingProducerConfig::new(2, 8, Duration::from_secs(60));
  let producer = BatchingProducer::new(PubSubTopic::from("news"), publisher, scheduler, config);

  let ack1 = producer.produce(AnyMessage::new(String::from("a"))).expect("produce");
  assert_eq!(ack1, PublishAck::accepted());

  let ack2 = producer.produce(AnyMessage::new(String::from("b"))).expect("produce");
  assert_eq!(ack2, PublishAck::accepted());

  assert_eq!(pubsub.batches(), vec![2]);
}

#[test]
fn rejects_when_queue_full() {
  let registry = make_registry();
  let pubsub = RecordingPubSub::new();
  let shared = ClusterPubSubShared::new(Box::new(pubsub));
  let publisher = PubSubPublisher::new(shared, registry);

  let system = new_noop_actor_system();
  let scheduler = system.state().scheduler();

  let config = BatchingProducerConfig::new(10, 1, Duration::from_secs(60));
  let producer = BatchingProducer::new(PubSubTopic::from("news"), publisher, scheduler, config);

  let _ = producer.produce(AnyMessage::new(String::from("a"))).expect("first");
  let ack = producer.produce(AnyMessage::new(String::from("b"))).expect("second");
  assert_eq!(ack, PublishAck::rejected(PublishRejectReason::QueueFull));
}
