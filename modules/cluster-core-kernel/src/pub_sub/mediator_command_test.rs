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
fn path_key_accepts_absolute_actor_selection() {
  let key = MediatorPathKey::parse("/user/service").expect("path");

  assert_eq!(key.as_str(), "/user/service");
}

#[test]
fn path_key_accepts_relative_actor_selection() {
  let key = MediatorPathKey::parse("../service").expect("path");

  assert_eq!(key.as_str(), "/user/service");
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
fn payload_commands_accept_empty_serialized_bytes() {
  let command = MediatorCommand::try_publish(PubSubTopic::new("news"), PubSubEnvelope {
    serializer_id: 41,
    type_name:     String::from("example.Message"),
    bytes:         vec![],
  })
  .expect("empty serialized bytes");

  assert!(matches!(command, MediatorCommand::Publish { .. }));
}

#[test]
fn payload_commands_accept_builtin_serializer_id() {
  let command = MediatorCommand::try_send(
    "fraktor://sys/user/service",
    PubSubEnvelope {
      serializer_id: 4,
      type_name:     String::from("alloc::string::String"),
      bytes:         vec![1, 2, 3],
    },
    false,
  )
  .expect("builtin serializer id");

  assert!(matches!(command, MediatorCommand::Send { .. }));
}

#[test]
fn payload_commands_reject_zero_serializer_id() {
  let error = MediatorCommand::try_send(
    "fraktor://sys/user/service",
    PubSubEnvelope { serializer_id: 0, type_name: String::from("example.Message"), bytes: vec![1, 2, 3] },
    false,
  )
  .expect_err("invalid payload");

  assert!(matches!(error, PubSubError::InvalidPayload { .. }));
}

#[test]
fn remove_command_keeps_target_identity() {
  let target = subscriber("actor-1");
  let command = MediatorCommand::try_remove("fraktor://sys/user/service", target.clone()).expect("command");

  match command {
    | MediatorCommand::Remove { path, target: actual } => {
      assert_eq!(path.as_str(), "/user/service");
      assert_eq!(actual, target);
    },
    | _ => panic!("unexpected command"),
  }
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
  let total = MediatorCommand::count();
  assert!(matches!(total, MediatorCommand::Query(MediatorQuery::Count)));

  let command = MediatorCommand::subscriber_count(PubSubTopic::new("news")).expect("query");

  match command {
    | MediatorCommand::Query(MediatorQuery::SubscriberCount { topic }) => assert_eq!(topic.as_str(), "news"),
    | _ => panic!("unexpected query"),
  }
}

#[test]
fn acknowledgement_preserves_subscription_operation_fields() {
  let acknowledgement = MediatorAcknowledgement::SubscribeCompleted {
    topic:      PubSubTopic::new("news"),
    group:      Some(String::from("blue")),
    subscriber: subscriber("sub-1"),
  };

  assert!(matches!(acknowledgement, MediatorAcknowledgement::SubscribeCompleted { .. }));
}

#[test]
fn query_result_preserves_snapshot_fields() {
  let total = MediatorQueryResult::Count { count: 2 };
  assert!(matches!(total, MediatorQueryResult::Count { count: 2 }));

  let result = MediatorQueryResult::CurrentTopics { topics: vec![PubSubTopic::new("news")] };

  assert!(matches!(result, MediatorQueryResult::CurrentTopics { .. }));
}
