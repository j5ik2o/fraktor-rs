use alloc::{string::String, vec, vec::Vec};

use fraktor_cluster_core_kernel_rs::{
  activation::ClusterIdentity,
  pub_sub::{
    DeliverBatchRequest, DeliveryEndpoint, DeliveryReport, DeliveryStatus, MediatorDeliveryIntent,
    MediatorDeliveryMode, MediatorPathKey, PubSubEnvelope, PubSubError, PubSubSubscriber, PubSubTopic,
    PubSubTopicOptions,
  },
};

use super::PubSubDeliveryIntentExecutor;

#[derive(Default)]
struct RecordingEndpoint {
  delivered: Vec<DeliverBatchRequest>,
}

impl DeliveryEndpoint for RecordingEndpoint {
  fn deliver(&mut self, request: DeliverBatchRequest) -> Result<DeliveryReport, PubSubError> {
    self.delivered.push(request);
    Ok(DeliveryReport { status: DeliveryStatus::Delivered, failed: vec![] })
  }
}

fn subscriber(name: &str) -> PubSubSubscriber {
  PubSubSubscriber::ClusterIdentity(ClusterIdentity::new("kind", name).expect("identity"))
}

fn payload() -> PubSubEnvelope {
  PubSubEnvelope { serializer_id: 41, type_name: String::from("example.Message"), bytes: vec![1] }
}

#[test]
fn executor_delivers_intent_targets_without_reselecting() {
  let topic = PubSubTopic::new("news");
  let targets = vec![subscriber("sub-1"), subscriber("sub-2")];
  let intent = MediatorDeliveryIntent::Deliver {
    mode:    MediatorDeliveryMode::Publish,
    targets: targets.clone(),
    payload: payload(),
  };
  let mut endpoint = RecordingEndpoint::default();

  let report = endpoint.execute_intent(topic.clone(), intent, PubSubTopicOptions::system_default()).expect("report");

  assert_eq!(report.status, DeliveryStatus::Delivered);
  assert_eq!(endpoint.delivered.len(), 1);
  assert_eq!(endpoint.delivered[0].topic, topic);
  assert_eq!(endpoint.delivered[0].subscribers, targets);
  assert_eq!(endpoint.delivered[0].batch.envelopes, vec![payload()]);
}

#[test]
fn executor_reports_drop_intent_as_delivered_without_endpoint() {
  let path = MediatorPathKey::parse("fraktor://sys/user/missing").expect("path");
  let mut endpoint = RecordingEndpoint::default();

  let report = endpoint
    .execute_intent(
      PubSubTopic::new("news"),
      MediatorDeliveryIntent::Dropped { path, payload: payload() },
      PubSubTopicOptions::system_default(),
    )
    .expect("report");

  assert_eq!(report.status, DeliveryStatus::Delivered);
  assert!(report.failed.is_empty());
  assert!(endpoint.delivered.is_empty());
}

#[test]
fn executor_reports_dead_letter_intent_as_error_without_endpoint() {
  let path = MediatorPathKey::parse("fraktor://sys/user/missing").expect("path");
  let mut endpoint = RecordingEndpoint::default();

  let error = match endpoint.execute_intent(
    PubSubTopic::new("news"),
    MediatorDeliveryIntent::DeadLetter { path, payload: payload() },
    PubSubTopicOptions::system_default(),
  ) {
    | Ok(_) => panic!("dead letter endpoint missing"),
    | Err(error) => error,
  };

  assert!(matches!(error, PubSubError::DeliveryFailed { .. }));
  assert!(endpoint.delivered.is_empty());
}

#[test]
fn executor_rejects_path_delivery_intent_without_topic_context() {
  let target = subscriber("actor-1");
  let mut endpoint = RecordingEndpoint::default();

  let error = match endpoint.execute_intent(
    PubSubTopic::new("news"),
    MediatorDeliveryIntent::Deliver { mode: MediatorDeliveryMode::Send, targets: vec![target], payload: payload() },
    PubSubTopicOptions::system_default(),
  ) {
    | Ok(_) => panic!("path intent"),
    | Err(error) => error,
  };

  assert!(matches!(error, PubSubError::DeliveryFailed { .. }));
  assert!(endpoint.delivered.is_empty());
}
