use fraktor_actor_rs::core::kernel::messaging::AnyMessage;
use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};

use super::PubSubPublisher;
use crate::core::{
  TopologyUpdate,
  pub_sub::{
    ClusterPubSubShared, PubSubError, PubSubSubscriber, PubSubTopic, PublishAck, PublishOptions, PublishRejectReason,
    PublishRequest, cluster_pub_sub::ClusterPubSub,
  },
};

#[derive(Clone)]
struct StubPubSub {
  publish_calls: ArcShared<NoStdMutex<usize>>,
}

impl StubPubSub {
  fn new() -> Self {
    Self { publish_calls: ArcShared::new(NoStdMutex::new(0)) }
  }

  fn publish_calls(&self) -> usize {
    *self.publish_calls.lock()
  }
}

impl ClusterPubSub for StubPubSub {
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

  fn publish(&mut self, _request: PublishRequest) -> Result<PublishAck, PubSubError> {
    *self.publish_calls.lock() += 1;
    Ok(PublishAck::accepted())
  }

  fn on_topology(&mut self, _update: &TopologyUpdate) {}
}

#[derive(Debug)]
struct CustomPayload;

#[test]
fn publish_rejects_when_not_serializable() {
  let setup = fraktor_actor_rs::core::kernel::serialization::default_serialization_setup();
  let registry = ArcShared::new(
    fraktor_actor_rs::core::kernel::serialization::serialization_registry::SerializationRegistry::from_setup(&setup),
  );
  let stub = StubPubSub::new();
  let shared = ClusterPubSubShared::new(Box::new(stub.clone()));
  let publisher = PubSubPublisher::new(shared, registry);

  let request =
    PublishRequest::new(PubSubTopic::from("news"), AnyMessage::new(CustomPayload), PublishOptions::default());
  let ack = publisher.publish(&request).expect("publish should return ack");
  assert_eq!(ack, PublishAck::rejected(PublishRejectReason::NotSerializable));
  assert_eq!(stub.publish_calls(), 0);
}
