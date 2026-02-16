use alloc::vec::Vec;
use core::time::Duration;

use fraktor_actor_rs::core::messaging::AnyMessageGeneric;
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdMutex, NoStdToolbox},
  sync::ArcShared,
};

use super::BatchingProducerGeneric;
use crate::core::{
  TopologyUpdate,
  pub_sub::{
    BatchingProducerConfig, ClusterPubSubShared, PubSubBatch, PubSubError, PubSubPublisherGeneric, PubSubSubscriber,
    PubSubTopic, PublishAck, PublishRejectReason, PublishRequest, cluster_pub_sub::ClusterPubSub,
  },
};

#[derive(Clone)]
struct RecordingPubSub {
  batches: ArcShared<NoStdMutex<Vec<usize>>>,
}

impl RecordingPubSub {
  fn new() -> Self {
    Self { batches: ArcShared::new(NoStdMutex::new(Vec::new())) }
  }

  fn batches(&self) -> Vec<usize> {
    self.batches.lock().clone()
  }
}

impl ClusterPubSub<NoStdToolbox> for RecordingPubSub {
  fn start(&mut self) -> Result<(), PubSubError> {
    Ok(())
  }

  fn stop(&mut self) -> Result<(), PubSubError> {
    Ok(())
  }

  fn subscribe(
    &mut self,
    _topic: &PubSubTopic,
    _subscriber: PubSubSubscriber<NoStdToolbox>,
  ) -> Result<(), PubSubError> {
    Ok(())
  }

  fn unsubscribe(
    &mut self,
    _topic: &PubSubTopic,
    _subscriber: PubSubSubscriber<NoStdToolbox>,
  ) -> Result<(), PubSubError> {
    Ok(())
  }

  fn publish(&mut self, request: PublishRequest<NoStdToolbox>) -> Result<PublishAck, PubSubError> {
    if let Some(batch) = request.payload.payload().downcast_ref::<PubSubBatch>() {
      self.batches.lock().push(batch.envelopes.len());
    }
    Ok(PublishAck::accepted())
  }

  fn on_topology(&mut self, _update: &TopologyUpdate) {}
}

fn make_registry()
-> ArcShared<fraktor_actor_rs::core::serialization::serialization_registry::SerializationRegistryGeneric<NoStdToolbox>>
{
  let setup = fraktor_actor_rs::core::serialization::default_serialization_setup();
  let registry = ArcShared::new(
    fraktor_actor_rs::core::serialization::serialization_registry::SerializationRegistryGeneric::from_setup(&setup),
  );
  let _ = fraktor_actor_rs::core::serialization::builtin::register_defaults(&registry, |_name, _id| {});
  registry
}

#[test]
fn flushes_when_batch_size_reached() {
  let registry = make_registry();
  let pubsub = RecordingPubSub::new();
  let shared = ClusterPubSubShared::new(Box::new(pubsub.clone()));
  let publisher = PubSubPublisherGeneric::new(shared, registry);

  let system = fraktor_actor_rs::core::system::ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let scheduler = system.state().scheduler();

  let config = BatchingProducerConfig::new(2, 8, Duration::from_secs(60));
  let producer = BatchingProducerGeneric::new(PubSubTopic::from("news"), publisher, scheduler, config);

  let ack1 = producer.produce(AnyMessageGeneric::new(String::from("a"))).expect("produce");
  assert_eq!(ack1, PublishAck::accepted());

  let ack2 = producer.produce(AnyMessageGeneric::new(String::from("b"))).expect("produce");
  assert_eq!(ack2, PublishAck::accepted());

  assert_eq!(pubsub.batches(), vec![2]);
}

#[test]
fn rejects_when_queue_full() {
  let registry = make_registry();
  let pubsub = RecordingPubSub::new();
  let shared = ClusterPubSubShared::new(Box::new(pubsub));
  let publisher = PubSubPublisherGeneric::new(shared, registry);

  let system = fraktor_actor_rs::core::system::ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let scheduler = system.state().scheduler();

  let config = BatchingProducerConfig::new(10, 1, Duration::from_secs(60));
  let producer = BatchingProducerGeneric::new(PubSubTopic::from("news"), publisher, scheduler, config);

  let _ = producer.produce(AnyMessageGeneric::new(String::from("a"))).expect("first");
  let ack = producer.produce(AnyMessageGeneric::new(String::from("b"))).expect("second");
  assert_eq!(ack, PublishAck::rejected(PublishRejectReason::QueueFull));
}
