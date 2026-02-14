use fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox;

use crate::core::{
  identity::ClusterIdentity,
  pub_sub::{
    DeliveryPolicy, PartitionBehavior, PubSubBroker, PubSubEvent, PubSubSubscriber, PubSubTopic, PubSubTopicOptions,
    PublishRejectReason,
  },
};

fn drain(broker: &mut PubSubBroker<NoStdToolbox>) -> Vec<PubSubEvent> {
  broker.drain_events()
}

#[test]
fn creates_topic_and_emits_event() {
  let mut broker: PubSubBroker<NoStdToolbox> = PubSubBroker::new();

  assert!(broker.create_topic(PubSubTopic::from("news")).is_ok());

  let events = drain(&mut broker);
  assert_eq!(events.len(), 1);
  assert!(matches!(events[0], PubSubEvent::TopicCreated { ref topic } if topic.as_str() == "news"));
}

#[test]
fn subscribes_existing_topic_and_records_event() {
  let mut broker: PubSubBroker<NoStdToolbox> = PubSubBroker::new();
  broker.create_topic(PubSubTopic::from("news")).expect("topic creation should succeed");

  let subscriber = PubSubSubscriber::ClusterIdentity(ClusterIdentity::new("kind", "pid-1").expect("identity"));
  assert!(broker.subscribe(&PubSubTopic::from("news"), &subscriber).is_ok());

  let events = drain(&mut broker);
  assert_eq!(events.len(), 2);
  assert!(matches!(events[0], PubSubEvent::TopicCreated { ref topic } if topic.as_str() == "news"));
  assert!(matches!(events[1], PubSubEvent::SubscriptionAdded { ref topic, ref subscriber }
      if topic.as_str() == "news" && subscriber == "kind/pid-1"));
}

#[test]
fn rejects_subscription_when_topic_missing() {
  let mut broker: PubSubBroker<NoStdToolbox> = PubSubBroker::new();
  let subscriber = PubSubSubscriber::ClusterIdentity(ClusterIdentity::new("kind", "pid-1").expect("identity"));
  broker.subscribe(&PubSubTopic::from("news"), &subscriber).expect("subscribe");

  let result = broker.subscribe(&PubSubTopic::from("news"), &subscriber);
  assert!(matches!(result, Err(crate::core::pub_sub::PubSubError::DuplicateSubscriber { .. })));

  let events = drain(&mut broker);
  assert!(events.iter().any(
    |event| matches!(event, PubSubEvent::SubscriptionRejected { reason, .. } if reason == "duplicate_subscriber")
  ));
}

#[test]
fn publish_fails_when_topic_missing() {
  let mut broker: PubSubBroker<NoStdToolbox> = PubSubBroker::new();
  let options = PubSubTopicOptions::system_default();

  let result = broker.publish_targets(&PubSubTopic::from("missing"), options);

  assert!(matches!(result, Err(PublishRejectReason::InvalidTopic)));

  let events = drain(&mut broker);
  assert_eq!(events.len(), 1);
  assert!(matches!(&events[0], PubSubEvent::PublishRejected { topic, reason }
    if topic.as_str() == "missing" && *reason == PublishRejectReason::InvalidTopic));
}

#[test]
fn publish_fails_when_no_subscribers() {
  let mut broker: PubSubBroker<NoStdToolbox> = PubSubBroker::new();
  broker.create_topic(PubSubTopic::from("news")).expect("topic creation should succeed");

  let options = PubSubTopicOptions::system_default();
  let result = broker.publish_targets(&PubSubTopic::from("news"), options);

  assert!(matches!(result, Err(PublishRejectReason::NoSubscribers)));

  let events = drain(&mut broker);
  assert_eq!(events.len(), 2);
  assert!(matches!(events[0], PubSubEvent::TopicCreated { ref topic } if topic.as_str() == "news"));
  assert!(matches!(&events[1], PubSubEvent::PublishRejected { topic, reason }
    if topic.as_str() == "news" && *reason == PublishRejectReason::NoSubscribers));
}

#[test]
fn at_most_once_drops_when_partitioned() {
  let mut broker: PubSubBroker<NoStdToolbox> = PubSubBroker::new();
  broker
    .create_topic_with_options(PubSubTopic::from("news"), PubSubTopicOptions {
      delivery_policy:    DeliveryPolicy::AtMostOnce,
      partition_behavior: PartitionBehavior::Drop,
    })
    .expect("topic creation should succeed");
  let subscriber = PubSubSubscriber::ClusterIdentity(ClusterIdentity::new("kind", "pid-1").expect("identity"));
  broker.subscribe(&PubSubTopic::from("news"), &subscriber).expect("subscription should succeed");

  broker.mark_partitioned(&PubSubTopic::from("news"), true).expect("partition flag update should succeed");

  let options = broker.topic_options(&PubSubTopic::from("news")).expect("topic options");
  let result = broker.publish_targets(&PubSubTopic::from("news"), options);

  assert!(matches!(result, Err(PublishRejectReason::PartitionDrop)));

  let events = drain(&mut broker);
  assert!(events.iter().any(|e| matches!(e, PubSubEvent::PublishDroppedDueToPartition { topic }
    if topic.as_str() == "news")));

  let metrics = broker.metrics();
  assert_eq!(metrics.dropped_messages, 1);
  assert_eq!(metrics.delayed_messages, 0);
}

#[test]
fn at_least_once_queues_and_flushes_after_recovery() {
  let mut broker: PubSubBroker<NoStdToolbox> = PubSubBroker::new();
  broker.create_topic(PubSubTopic::from("news")).expect("topic creation should succeed");
  let subscriber = PubSubSubscriber::ClusterIdentity(ClusterIdentity::new("kind", "pid-1").expect("identity"));
  broker.subscribe(&PubSubTopic::from("news"), &subscriber).expect("subscription should succeed");

  broker.mark_partitioned(&PubSubTopic::from("news"), true).expect("partition flag update should succeed");

  let result = broker
    .publish_targets(&PubSubTopic::from("news"), PubSubTopicOptions::system_default())
    .expect("queueing should not be an error");
  assert!(result.is_empty());

  let mut events = drain(&mut broker);
  assert!(events.iter().any(|e| matches!(e, PubSubEvent::PublishQueuedDueToPartition { topic }
    if topic.as_str() == "news")));

  let metrics = broker.metrics();
  assert_eq!(metrics.delayed_messages, 1);

  broker.mark_partitioned(&PubSubTopic::from("news"), false).expect("clearing partition should succeed");

  events = drain(&mut broker);
  assert!(events.iter().any(|e| matches!(
    e,
    PubSubEvent::PublishQueuedFlushed { topic, count } if topic.as_str() == "news" && *count == 1
  )));

  let metrics = broker.metrics();
  assert_eq!(metrics.redelivered_messages, 1);
  assert_eq!(metrics.dropped_messages, 0);
}

#[test]
fn metrics_snapshot_is_emitted_and_counters_reset() {
  let mut broker: PubSubBroker<NoStdToolbox> = PubSubBroker::new();
  broker
    .create_topic_with_options(PubSubTopic::from("news"), PubSubTopicOptions {
      delivery_policy:    DeliveryPolicy::AtMostOnce,
      partition_behavior: PartitionBehavior::Drop,
    })
    .expect("topic creation should succeed");
  let subscriber = PubSubSubscriber::ClusterIdentity(ClusterIdentity::new("kind", "pid-1").expect("identity"));
  broker.subscribe(&PubSubTopic::from("news"), &subscriber).expect("subscription should succeed");

  broker.mark_partitioned(&PubSubTopic::from("news"), true).expect("partition flag update should succeed");
  let options = broker.topic_options(&PubSubTopic::from("news")).expect("topic options");
  let _ = broker.publish_targets(&PubSubTopic::from("news"), options);

  let snapshot = broker.drain_metrics();
  assert_eq!(snapshot.delayed_messages, 0);
  assert_eq!(snapshot.dropped_messages, 1);
  assert_eq!(snapshot.redelivered_messages, 0);

  let events = drain(&mut broker);
  assert!(events.iter().any(|e| matches!(
    e,
    PubSubEvent::MetricsSnapshot {
      delayed_messages,
      dropped_messages,
      redelivered_messages,
    } if *delayed_messages == 0 && *dropped_messages == 1 && *redelivered_messages == 0
  )));
  assert!(events.iter().any(|e| matches!(
    e,
    PubSubEvent::MetricsSnapshotByTopic { snapshots }
      if snapshots.len() == 1 && snapshots[0].0.as_str() == "news" && snapshots[0].1.dropped_messages == 1
  )));

  let metrics_after = broker.metrics();
  assert_eq!(metrics_after.delayed_messages, 0);
  assert_eq!(metrics_after.dropped_messages, 0);
  assert_eq!(metrics_after.redelivered_messages, 0);
}
