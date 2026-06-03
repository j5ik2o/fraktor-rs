use alloc::{string::String, vec};

use crate::{
  activation::ClusterIdentity,
  pub_sub::{
    MediatorAcknowledgement, MediatorCommand, MediatorPathKey, MediatorQuery, MediatorQueryResult, PubSubEnvelope,
    PubSubError, PubSubSubscriber, PubSubTopic,
  },
};

fn subscriber(name: &str) -> PubSubSubscriber {
  PubSubSubscriber::ClusterIdentity(ClusterIdentity::new("kind", name).expect("identity"))
}

fn payload() -> PubSubEnvelope {
  PubSubEnvelope { serializer_id: 41, type_name: String::from("example.Message"), bytes: vec![1, 2, 3] }
}

#[test]
fn path_key_uses_address_less_relative_actor_path() {
  let left = MediatorPathKey::parse("fraktor.tcp://sys@node-a:2552/user/service").expect("path");
  let right = MediatorPathKey::parse("fraktor.tcp://sys@node-b:2553/user/service").expect("path");

  assert_eq!(left.as_str(), "/user/service");
  assert_eq!(left, right);
}

#[test]
fn path_command_rejects_invalid_path() {
  let error = MediatorCommand::try_put("", subscriber("actor-1")).expect_err("invalid path");

  assert!(matches!(error, PubSubError::InvalidPath { .. }));
}

#[test]
fn path_command_rejects_guardian_only_path() {
  let error = MediatorCommand::try_send("fraktor://sys", payload(), false).expect_err("empty actor path");

  assert!(matches!(error, PubSubError::InvalidPath { .. }));
}

#[test]
fn topic_commands_reject_empty_topic() {
  let error =
    MediatorCommand::try_subscribe(PubSubTopic::new(""), None, subscriber("sub-1")).expect_err("invalid topic");

  assert!(matches!(error, PubSubError::InvalidTopic { .. }));
}

#[test]
fn payload_commands_reject_empty_payload() {
  let error = MediatorCommand::try_publish(PubSubTopic::new("news"), PubSubEnvelope {
    serializer_id: 41,
    type_name:     String::from("example.Message"),
    bytes:         vec![],
  })
  .expect_err("invalid payload");

  assert!(matches!(error, PubSubError::InvalidPayload { .. }));
}

#[test]
fn payload_commands_reject_reserved_serializer_id() {
  let error = MediatorCommand::try_send(
    "fraktor://sys/user/service",
    PubSubEnvelope { serializer_id: 40, type_name: String::from("example.Message"), bytes: vec![1, 2, 3] },
    false,
  )
  .expect_err("invalid payload");

  assert!(matches!(error, PubSubError::InvalidPayload { .. }));
}

#[test]
fn command_constructors_keep_protocol_fields() {
  let command = MediatorCommand::try_send_to_all("fraktor://sys/user/service", payload(), true).expect("command");

  match command {
    | MediatorCommand::SendToAll { path, all_but_self, .. } => {
      assert_eq!(path.as_str(), "/user/service");
      assert!(all_but_self);
    },
    | _ => panic!("unexpected command"),
  }
}

#[test]
fn query_commands_keep_requested_shape() {
  let command = MediatorCommand::subscriber_count(PubSubTopic::new("news")).expect("query");

  match command {
    | MediatorCommand::Query(MediatorQuery::SubscriberCount { topic }) => assert_eq!(topic.as_str(), "news"),
    | _ => panic!("unexpected query"),
  }
}

#[test]
fn acknowledgement_reports_completed_subscription_operation() {
  let acknowledgement = MediatorAcknowledgement::SubscribeCompleted {
    topic:      PubSubTopic::new("news"),
    group:      Some(String::from("blue")),
    subscriber: subscriber("sub-1"),
  };

  assert!(acknowledgement.is_completed());
}

#[test]
fn query_result_reports_completed_snapshot() {
  let result = MediatorQueryResult::CurrentTopics { topics: vec![PubSubTopic::new("news")] };

  assert!(result.is_completed());
}
