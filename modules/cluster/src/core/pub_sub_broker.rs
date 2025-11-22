//! In-memory topic management for cluster-wide pub/sub.

use alloc::{
  collections::{BTreeMap, BTreeSet},
  string::{String, ToString},
  vec::Vec,
};

use crate::core::{
  delivery_policy::DeliveryPolicy, partition_behavior::PartitionBehavior, pub_sub_error::PubSubError,
  pub_sub_event::PubSubEvent, pub_sub_metrics::PubSubMetrics, pub_sub_topic_metrics::PubSubTopicMetrics,
};

#[cfg(test)]
mod tests;

#[derive(Debug, Default)]
struct TopicEntry {
  subscribers:          BTreeSet<String>,
  policy:               DeliveryPolicy,
  partition_behavior:   PartitionBehavior,
  partitioned:          bool,
  queued_message_count: usize,
}

/// Simple pub/sub broker that tracks topics and subscriptions.
pub struct PubSubBroker {
  topics:        BTreeMap<String, TopicEntry>,
  events:        Vec<PubSubEvent>,
  metrics:       PubSubMetrics,
  topic_metrics: BTreeMap<String, PubSubTopicMetrics>,
}

impl Default for PubSubBroker {
  fn default() -> Self {
    Self::new()
  }
}

impl PubSubBroker {
  /// Creates an empty broker.
  #[must_use]
  pub const fn new() -> Self {
    Self {
      topics:        BTreeMap::new(),
      events:        Vec::new(),
      metrics:       PubSubMetrics::new(),
      topic_metrics: BTreeMap::new(),
    }
  }

  /// Registers a new topic and emits a creation event.
  ///
  /// # Errors
  ///
  /// Returns [`PubSubError::TopicAlreadyExists`] if the topic already exists.
  pub fn create_topic(&mut self, topic: String) -> Result<(), PubSubError> {
    self.create_topic_with_options(topic, DeliveryPolicy::AtLeastOnce, PartitionBehavior::DelayQueue)
  }

  /// Registers a new topic with explicit delivery and partition policies.
  ///
  /// # Errors
  ///
  /// Returns [`PubSubError::TopicAlreadyExists`] if the topic already exists.
  pub fn create_topic_with_options(
    &mut self,
    topic: String,
    policy: DeliveryPolicy,
    partition_behavior: PartitionBehavior,
  ) -> Result<(), PubSubError> {
    if self.topics.contains_key(&topic) {
      self.events.push(PubSubEvent::TopicAlreadyExists { topic: topic.clone() });
      return Err(PubSubError::TopicAlreadyExists { topic });
    }

    self.topics.insert(topic.clone(), TopicEntry {
      subscribers: BTreeSet::new(),
      policy,
      partition_behavior,
      partitioned: false,
      queued_message_count: 0,
    });
    self.topic_metrics.entry(topic.clone()).or_default();
    self.events.push(PubSubEvent::TopicCreated { topic });

    Ok(())
  }

  /// Adds a subscriber to an existing topic.
  ///
  /// # Errors
  ///
  /// Returns [`PubSubError::TopicNotFound`] if the topic does not exist.
  /// Returns [`PubSubError::DuplicateSubscriber`] if the subscriber is already registered.
  pub fn subscribe(&mut self, topic: &str, subscriber: String) -> Result<(), PubSubError> {
    let Some(entry) = self.topics.get_mut(topic) else {
      self.events.push(PubSubEvent::SubscriptionRejected {
        topic: topic.to_string(),
        subscriber,
        reason: "topic_missing".to_string(),
      });
      return Err(PubSubError::TopicNotFound { topic: topic.to_string() });
    };

    if entry.subscribers.contains(&subscriber) {
      self.events.push(PubSubEvent::SubscriptionRejected {
        topic:      topic.to_string(),
        subscriber: subscriber.clone(),
        reason:     "duplicate_subscriber".to_string(),
      });
      return Err(PubSubError::DuplicateSubscriber { topic: topic.to_string(), subscriber });
    }

    entry.subscribers.insert(subscriber.clone());
    self.events.push(PubSubEvent::SubscriptionAccepted { topic: topic.to_string(), subscriber });

    Ok(())
  }

  /// Validates publish readiness and returns subscribers.
  ///
  /// # Errors
  ///
  /// Returns [`PubSubError::TopicNotFound`] if the topic does not exist.
  /// Returns [`PubSubError::NoSubscribers`] if there are no subscribers to the topic.
  /// Returns [`PubSubError::PartitionDrop`] if the topic is partitioned and the policy drops
  /// messages.
  pub fn publish(&mut self, topic: &str) -> Result<Vec<String>, PubSubError> {
    let Some(entry) = self.topics.get(topic) else {
      self.events.push(PubSubEvent::PublishRejectedMissingTopic { topic: topic.to_string() });
      return Err(PubSubError::TopicNotFound { topic: topic.to_string() });
    };

    if entry.partitioned {
      return self.handle_partitioned_publish(topic);
    }

    if entry.subscribers.is_empty() {
      self.events.push(PubSubEvent::PublishRejectedNoSubscribers { topic: topic.to_string() });
      return Err(PubSubError::NoSubscribers { topic: topic.to_string() });
    }

    Ok(entry.subscribers.iter().cloned().collect())
  }

  /// Marks or clears partition state. Returns the number of flushed queued messages when recovered.
  ///
  /// # Errors
  ///
  /// Returns [`PubSubError::TopicNotFound`] if the topic does not exist.
  pub fn mark_partitioned(&mut self, topic: &str, partitioned: bool) -> Result<usize, PubSubError> {
    let Some(entry) = self.topics.get_mut(topic) else {
      self.events.push(PubSubEvent::PublishRejectedMissingTopic { topic: topic.to_string() });
      return Err(PubSubError::TopicNotFound { topic: topic.to_string() });
    };

    if partitioned {
      entry.partitioned = true;
      self.events.push(PubSubEvent::PartitionMarked { topic: topic.to_string() });
      return Ok(0);
    }

    entry.partitioned = false;
    let flushed = entry.queued_message_count;
    let subscribers_empty = entry.subscribers.is_empty();
    entry.queued_message_count = 0;

    if flushed > 0 {
      if subscribers_empty {
        self.metrics.dropped_messages = self.metrics.dropped_messages.saturating_add(flushed as u64);
        self.bump_topic_metrics(topic, |m| m.dropped_messages = m.dropped_messages.saturating_add(flushed as u64));
        self.events.push(PubSubEvent::PublishDroppedDueToPartition { topic: topic.to_string() });
      } else {
        self.metrics.redelivered_messages = self.metrics.redelivered_messages.saturating_add(flushed as u64);
        self.bump_topic_metrics(topic, |m| {
          m.redelivered_messages = m.redelivered_messages.saturating_add(flushed as u64)
        });
        self.events.push(PubSubEvent::PublishQueuedFlushed { topic: topic.to_string(), count: flushed });
      }
    }

    self.events.push(PubSubEvent::PartitionRecovered { topic: topic.to_string() });
    Ok(flushed)
  }

  /// Returns current metrics snapshot.
  #[must_use]
  pub const fn metrics(&self) -> PubSubMetrics {
    self.metrics
  }

  /// Emits a metrics snapshot event and resets counters.
  pub fn drain_metrics(&mut self) -> PubSubMetrics {
    let snapshot = self.metrics;
    let topic_snapshot: Vec<(String, PubSubTopicMetrics)> =
      self.topic_metrics.iter().map(|(k, v)| (k.clone(), *v)).collect();
    self.events.push(PubSubEvent::MetricsSnapshot {
      delayed_messages:     snapshot.delayed_messages,
      dropped_messages:     snapshot.dropped_messages,
      redelivered_messages: snapshot.redelivered_messages,
    });
    self.events.push(PubSubEvent::MetricsSnapshotByTopic { snapshots: topic_snapshot });
    self.metrics = PubSubMetrics::new();
    self.topic_metrics.clear();
    snapshot
  }

  /// Drains buffered events.
  pub fn drain_events(&mut self) -> Vec<PubSubEvent> {
    core::mem::take(&mut self.events)
  }

  fn handle_partitioned_publish(&mut self, topic: &str) -> Result<Vec<String>, PubSubError> {
    let (policy, behavior) = {
      let Some(entry) = self.topics.get_mut(topic) else {
        self.events.push(PubSubEvent::PublishRejectedMissingTopic { topic: topic.to_string() });
        return Err(PubSubError::TopicNotFound { topic: topic.to_string() });
      };

      let policy = entry.policy;
      let behavior = entry.partition_behavior;
      if matches!((policy, behavior), (DeliveryPolicy::AtLeastOnce, PartitionBehavior::DelayQueue)) {
        entry.queued_message_count = entry.queued_message_count.saturating_add(1);
      }

      (policy, behavior)
    };

    match (policy, behavior) {
      | (DeliveryPolicy::AtMostOnce, _) | (_, PartitionBehavior::Drop) => {
        self.metrics.dropped_messages = self.metrics.dropped_messages.saturating_add(1);
        self.bump_topic_metrics(topic, |m| m.dropped_messages = m.dropped_messages.saturating_add(1));
        self.events.push(PubSubEvent::PublishDroppedDueToPartition { topic: topic.to_string() });
        Err(PubSubError::PartitionDrop { topic: topic.to_string() })
      },
      | (DeliveryPolicy::AtLeastOnce, PartitionBehavior::DelayQueue) => {
        self.metrics.delayed_messages = self.metrics.delayed_messages.saturating_add(1);
        self.bump_topic_metrics(topic, |m| m.delayed_messages = m.delayed_messages.saturating_add(1));
        self.events.push(PubSubEvent::PublishQueuedDueToPartition { topic: topic.to_string() });
        Ok(Vec::new())
      },
    }
  }

  fn bump_topic_metrics<F>(&mut self, topic: &str, mut f: F)
  where
    F: FnMut(&mut PubSubTopicMetrics), {
    let entry = self.topic_metrics.entry(topic.to_string()).or_default();
    f(entry);
  }
}
