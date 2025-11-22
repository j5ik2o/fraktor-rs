use crate::core::{
  delivery_policy::DeliveryPolicy, partition_behavior::PartitionBehavior, pub_sub_broker::PubSubBroker,
  pub_sub_error::PubSubError, pub_sub_event::PubSubEvent,
};

fn drain(broker: &mut PubSubBroker) -> Vec<PubSubEvent> {
  broker.drain_events()
}

#[test]
fn creates_topic_and_emits_event() {
  let mut broker = PubSubBroker::new();

  assert!(broker.create_topic("news".to_string()).is_ok());

  let events = drain(&mut broker);
  assert_eq!(events.len(), 1);
  assert!(matches!(events[0], PubSubEvent::TopicCreated { ref topic } if topic == "news"));
}

#[test]
fn subscribes_existing_topic_and_records_event() {
  let mut broker = PubSubBroker::new();
  broker.create_topic("news".to_string()).expect("topic creation should succeed");

  assert!(broker.subscribe("news", "pid-1".to_string()).is_ok());

  let events = drain(&mut broker);
  assert_eq!(events.len(), 2);
  assert!(matches!(events[0], PubSubEvent::TopicCreated { ref topic } if topic == "news"));
  assert!(
    matches!(events[1], PubSubEvent::SubscriptionAccepted { ref topic, ref subscriber } if topic == "news" && subscriber == "pid-1")
  );
}

#[test]
fn rejects_subscription_when_topic_missing() {
  let mut broker = PubSubBroker::new();

  let result = broker.subscribe("missing", "pid-1".to_string());

  assert!(matches!(result, Err(PubSubError::TopicNotFound { ref topic }) if topic == "missing"));

  let events = drain(&mut broker);
  assert_eq!(events.len(), 1);
  assert!(
    matches!(events[0], PubSubEvent::SubscriptionRejected { ref topic, ref reason, .. } if topic == "missing" && reason == "topic_missing")
  );
}

#[test]
fn publish_fails_when_topic_missing() {
  let mut broker = PubSubBroker::new();

  let result = broker.publish("missing");

  assert!(matches!(result, Err(PubSubError::TopicNotFound { ref topic }) if topic == "missing"));

  let events = drain(&mut broker);
  assert_eq!(events.len(), 1);
  assert!(matches!(events[0], PubSubEvent::PublishRejectedMissingTopic { ref topic } if topic == "missing"));
}

#[test]
fn publish_fails_when_no_subscribers() {
  let mut broker = PubSubBroker::new();
  broker.create_topic("news".to_string()).expect("topic creation should succeed");

  let result = broker.publish("news");

  assert!(matches!(result, Err(PubSubError::NoSubscribers { ref topic }) if topic == "news"));

  let events = drain(&mut broker);
  assert_eq!(events.len(), 2);
  assert!(matches!(events[0], PubSubEvent::TopicCreated { ref topic } if topic == "news"));
  assert!(matches!(events[1], PubSubEvent::PublishRejectedNoSubscribers { ref topic } if topic == "news"));
}

#[test]
fn at_most_once_drops_when_partitioned() {
  let mut broker = PubSubBroker::new();
  broker
    .create_topic_with_options("news".to_string(), DeliveryPolicy::AtMostOnce, PartitionBehavior::Drop)
    .expect("topic creation should succeed");
  broker.subscribe("news", "pid-1".to_string()).expect("subscription should succeed");

  broker.mark_partitioned("news", true).expect("partition flag update should succeed");

  let result = broker.publish("news");

  assert!(matches!(result, Err(PubSubError::PartitionDrop { topic }) if topic == "news"));

  let events = drain(&mut broker);
  assert!(events.iter().any(|e| matches!(e, PubSubEvent::PublishDroppedDueToPartition { topic } if topic == "news")));

  let metrics = broker.metrics();
  assert_eq!(metrics.dropped_messages, 1);
  assert_eq!(metrics.delayed_messages, 0);
}

#[test]
fn at_least_once_queues_and_flushes_after_recovery() {
  let mut broker = PubSubBroker::new();
  broker.create_topic("news".to_string()).expect("topic creation should succeed");
  broker.subscribe("news", "pid-1".to_string()).expect("subscription should succeed");

  broker.mark_partitioned("news", true).expect("partition flag update should succeed");

  let result = broker.publish("news").expect("queueing should not be an error");
  assert!(result.is_empty());

  let mut events = drain(&mut broker);
  assert!(events.iter().any(|e| matches!(e, PubSubEvent::PublishQueuedDueToPartition { topic } if topic == "news")));

  let metrics = broker.metrics();
  assert_eq!(metrics.delayed_messages, 1);

  broker.mark_partitioned("news", false).expect("clearing partition should succeed");

  events = drain(&mut broker);
  assert!(events.iter().any(|e| matches!(
    e,
    PubSubEvent::PublishQueuedFlushed { topic, count } if topic == "news" && *count == 1
  )));

  let metrics = broker.metrics();
  assert_eq!(metrics.redelivered_messages, 1);
  assert_eq!(metrics.dropped_messages, 0);
}

#[test]
fn metrics_snapshot_is_emitted_and_counters_reset() {
  let mut broker = PubSubBroker::new();
  broker
    .create_topic_with_options("news".to_string(), DeliveryPolicy::AtMostOnce, PartitionBehavior::Drop)
    .expect("topic creation should succeed");
  broker.subscribe("news", "pid-1".to_string()).expect("subscription should succeed");

  broker.mark_partitioned("news", true).expect("partition flag update should succeed");
  let _ = broker.publish("news");

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
      if snapshots.len() == 1 && snapshots[0].0 == "news" && snapshots[0].1.dropped_messages == 1
  )));

  let metrics_after = broker.metrics();
  assert_eq!(metrics_after.delayed_messages, 0);
  assert_eq!(metrics_after.dropped_messages, 0);
  assert_eq!(metrics_after.redelivered_messages, 0);
}
