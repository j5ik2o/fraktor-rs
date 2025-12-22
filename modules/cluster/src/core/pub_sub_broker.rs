//! In-memory topic management for cluster-wide pub/sub.

use alloc::{
  collections::BTreeMap,
  string::{String, ToString},
  vec::Vec,
};
use core::time::Duration;

use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, time::TimerInstant};

use crate::core::{
  DeliveryPolicy, PartitionBehavior, PubSubError, PubSubEvent, PubSubMetrics, PubSubSubscriber, PubSubTopic,
  PubSubTopicMetrics, PubSubTopicOptions, PublishRejectReason, SubscriberState,
};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone)]
struct SubscriberRecord {
  state:        SubscriberState,
  suspended_at: Option<TimerInstant>,
}

impl SubscriberRecord {
  const fn active() -> Self {
    Self { state: SubscriberState::Active, suspended_at: None }
  }

  fn suspend(&mut self, reason: String, now: TimerInstant) {
    self.state = SubscriberState::Suspended { reason };
    self.suspended_at = Some(now);
  }

  fn activate(&mut self) {
    self.state = SubscriberState::Active;
    self.suspended_at = None;
  }

  const fn is_active(&self) -> bool {
    matches!(self.state, SubscriberState::Active)
  }

  fn is_expired(&self, now: TimerInstant, ttl: Duration) -> bool {
    let Some(at) = self.suspended_at else {
      return false;
    };
    if at.resolution() != now.resolution() {
      return false;
    }
    let ttl_ticks = ttl_ticks(ttl, at.resolution());
    let elapsed = now.ticks().saturating_sub(at.ticks());
    elapsed >= ttl_ticks
  }
}

#[derive(Debug)]
struct TopicEntry<TB: RuntimeToolbox> {
  subscriber_state:     BTreeMap<PubSubSubscriber<TB>, SubscriberRecord>,
  options:              PubSubTopicOptions,
  partitioned:          bool,
  queued_message_count: usize,
}

/// Simple pub/sub broker that tracks topics and subscriptions.
pub struct PubSubBroker<TB: RuntimeToolbox> {
  topics:        BTreeMap<PubSubTopic, TopicEntry<TB>>,
  events:        Vec<PubSubEvent>,
  metrics:       PubSubMetrics,
  topic_metrics: BTreeMap<PubSubTopic, PubSubTopicMetrics>,
}

impl<TB: RuntimeToolbox> Default for PubSubBroker<TB> {
  fn default() -> Self {
    Self::new()
  }
}

impl<TB: RuntimeToolbox> PubSubBroker<TB> {
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
  pub fn create_topic(&mut self, topic: PubSubTopic) -> Result<(), PubSubError> {
    self.create_topic_with_options(topic, PubSubTopicOptions::system_default())
  }

  /// Registers a new topic with explicit delivery and partition policies.
  ///
  /// # Errors
  ///
  /// Returns [`PubSubError::TopicAlreadyExists`] if the topic already exists.
  pub fn create_topic_with_options(
    &mut self,
    topic: PubSubTopic,
    options: PubSubTopicOptions,
  ) -> Result<(), PubSubError> {
    if self.topics.contains_key(&topic) {
      self.events.push(PubSubEvent::TopicAlreadyExists { topic: topic.clone() });
      return Err(PubSubError::TopicAlreadyExists { topic });
    }

    self.topics.insert(topic.clone(), TopicEntry {
      subscriber_state: BTreeMap::new(),
      options,
      partitioned: false,
      queued_message_count: 0,
    });
    self.topic_metrics.entry(topic.clone()).or_default();
    self.events.push(PubSubEvent::TopicCreated { topic });

    Ok(())
  }

  /// Returns the default options for the topic.
  ///
  /// # Errors
  ///
  /// Returns [`PubSubError::TopicNotFound`] if the topic does not exist.
  pub fn topic_options(&self, topic: &PubSubTopic) -> Result<PubSubTopicOptions, PubSubError> {
    self.topics.get(topic).map(|entry| entry.options).ok_or_else(|| PubSubError::TopicNotFound { topic: topic.clone() })
  }

  /// Adds a subscriber to an existing topic.
  ///
  /// # Errors
  ///
  /// Returns [`PubSubError::TopicNotFound`] if the topic does not exist.
  /// Returns [`PubSubError::DuplicateSubscriber`] if the subscriber is already registered.
  pub fn subscribe(&mut self, topic: &PubSubTopic, subscriber: &PubSubSubscriber<TB>) -> Result<(), PubSubError> {
    if !self.topics.contains_key(topic) {
      let _ = self.create_topic(topic.clone());
    }
    let Some(entry) = self.topics.get_mut(topic) else {
      self.events.push(PubSubEvent::SubscriptionRejected {
        topic:      topic.clone(),
        subscriber: subscriber.label(),
        reason:     "topic_missing".to_string(),
      });
      return Err(PubSubError::TopicNotFound { topic: topic.clone() });
    };

    if entry.subscriber_state.contains_key(subscriber) {
      self.events.push(PubSubEvent::SubscriptionRejected {
        topic:      topic.clone(),
        subscriber: subscriber.label(),
        reason:     "duplicate_subscriber".to_string(),
      });
      return Err(PubSubError::DuplicateSubscriber { topic: topic.clone(), subscriber: subscriber.label() });
    }

    entry.subscriber_state.insert(subscriber.clone(), SubscriberRecord::active());
    self.events.push(PubSubEvent::SubscriptionAdded { topic: topic.clone(), subscriber: subscriber.label() });

    Ok(())
  }

  /// Removes a subscriber from an existing topic.
  ///
  /// # Errors
  ///
  /// Returns [`PubSubError::TopicNotFound`] if the topic does not exist.
  /// Returns [`PubSubError::SubscriberNotFound`] if the subscriber is not registered.
  pub fn unsubscribe(&mut self, topic: &PubSubTopic, subscriber: &PubSubSubscriber<TB>) -> Result<(), PubSubError> {
    let Some(entry) = self.topics.get_mut(topic) else {
      return Err(PubSubError::TopicNotFound { topic: topic.clone() });
    };

    if entry.subscriber_state.remove(subscriber).is_none() {
      return Err(PubSubError::SubscriberNotFound { topic: topic.clone(), subscriber: subscriber.label() });
    }

    self.events.push(PubSubEvent::SubscriptionRemoved {
      topic:      topic.clone(),
      subscriber: subscriber.label(),
      reason:     "unsubscribed".to_string(),
    });

    Ok(())
  }

  /// Returns active subscribers for the topic.
  ///
  /// # Errors
  ///
  /// Returns [`PubSubError::TopicNotFound`] if the topic does not exist.
  pub fn active_subscribers(&self, topic: &PubSubTopic) -> Result<Vec<PubSubSubscriber<TB>>, PubSubError> {
    let Some(entry) = self.topics.get(topic) else {
      return Err(PubSubError::TopicNotFound { topic: topic.clone() });
    };
    Ok(
      entry
        .subscriber_state
        .iter()
        .filter(|(_, record)| record.is_active())
        .map(|(subscriber, _)| (*subscriber).clone())
        .collect(),
    )
  }

  /// Suspends a subscriber due to delivery failure.
  ///
  /// # Errors
  ///
  /// Returns [`PubSubError::TopicNotFound`] if the topic does not exist.
  pub fn suspend_subscriber(
    &mut self,
    topic: &PubSubTopic,
    subscriber: &PubSubSubscriber<TB>,
    reason: impl Into<String>,
    now: TimerInstant,
  ) -> Result<(), PubSubError> {
    let Some(entry) = self.topics.get_mut(topic) else {
      return Err(PubSubError::TopicNotFound { topic: topic.clone() });
    };
    if let Some(record) = entry.subscriber_state.get_mut(subscriber) {
      record.suspend(reason.into(), now);
    }
    Ok(())
  }

  /// Reactivates all suspended subscribers for the topic.
  ///
  /// # Errors
  ///
  /// Returns [`PubSubError::TopicNotFound`] if the topic does not exist.
  pub fn reactivate_all(&mut self, topic: &PubSubTopic) -> Result<Vec<PubSubSubscriber<TB>>, PubSubError> {
    let Some(entry) = self.topics.get_mut(topic) else {
      return Err(PubSubError::TopicNotFound { topic: topic.clone() });
    };
    let mut reactivated = Vec::new();
    for (subscriber, record) in entry.subscriber_state.iter_mut() {
      if !record.is_active() {
        record.activate();
        reactivated.push((*subscriber).clone());
      }
    }
    Ok(reactivated)
  }

  /// Removes suspended subscribers whose TTL elapsed.
  ///
  /// # Errors
  ///
  /// Returns [`PubSubError::TopicNotFound`] if the topic does not exist.
  pub fn remove_expired_suspended(
    &mut self,
    topic: &PubSubTopic,
    now: TimerInstant,
    ttl: Duration,
  ) -> Result<Vec<PubSubSubscriber<TB>>, PubSubError> {
    let Some(entry) = self.topics.get_mut(topic) else {
      return Err(PubSubError::TopicNotFound { topic: topic.clone() });
    };
    let mut removed = Vec::new();
    entry.subscriber_state.retain(|subscriber, record| {
      if record.is_expired(now, ttl) {
        removed.push((*subscriber).clone());
        false
      } else {
        true
      }
    });
    Ok(removed)
  }

  /// Validates publish readiness and returns active subscribers.
  ///
  /// # Errors
  ///
  /// Returns [`PublishRejectReason`] when the topic is missing, has no subscribers,
  /// or delivery is blocked by a partition policy.
  pub fn publish_targets(
    &mut self,
    topic: &PubSubTopic,
    options: PubSubTopicOptions,
  ) -> Result<Vec<PubSubSubscriber<TB>>, PublishRejectReason> {
    let Some(entry) = self.topics.get_mut(topic) else {
      self
        .events
        .push(PubSubEvent::PublishRejected { topic: topic.clone(), reason: PublishRejectReason::InvalidTopic });
      return Err(PublishRejectReason::InvalidTopic);
    };

    if entry.partitioned {
      return self.handle_partitioned_publish(topic, options);
    }

    let subscribers: Vec<_> = entry
      .subscriber_state
      .iter()
      .filter(|(_, record)| record.is_active())
      .map(|(subscriber, _)| (*subscriber).clone())
      .collect();
    if subscribers.is_empty() {
      self
        .events
        .push(PubSubEvent::PublishRejected { topic: topic.clone(), reason: PublishRejectReason::NoSubscribers });
      return Err(PublishRejectReason::NoSubscribers);
    }

    self
      .events
      .push(PubSubEvent::PublishAccepted { topic: topic.clone(), subscriber_count: subscribers.len() });
    Ok(subscribers)
  }

  /// Marks or clears partition state. Returns the number of flushed queued messages when recovered.
  ///
  /// # Errors
  ///
  /// Returns [`PubSubError::TopicNotFound`] if the topic does not exist.
  pub fn mark_partitioned(&mut self, topic: &PubSubTopic, partitioned: bool) -> Result<usize, PubSubError> {
    let Some(entry) = self.topics.get_mut(topic) else {
      return Err(PubSubError::TopicNotFound { topic: topic.clone() });
    };

    if partitioned {
      entry.partitioned = true;
      self.events.push(PubSubEvent::PartitionMarked { topic: topic.clone() });
      return Ok(0);
    }

    entry.partitioned = false;
    let flushed = entry.queued_message_count;
    let subscribers_empty = entry.subscriber_state.values().all(|record| !record.is_active());
    entry.queued_message_count = 0;

    if flushed > 0 {
      if subscribers_empty {
        self.metrics.dropped_messages = self.metrics.dropped_messages.saturating_add(flushed as u64);
        self.bump_topic_metrics(topic, |m| m.dropped_messages = m.dropped_messages.saturating_add(flushed as u64));
        self.events.push(PubSubEvent::PublishDroppedDueToPartition { topic: topic.clone() });
      } else {
        self.metrics.redelivered_messages = self.metrics.redelivered_messages.saturating_add(flushed as u64);
        self.bump_topic_metrics(topic, |m| {
          m.redelivered_messages = m.redelivered_messages.saturating_add(flushed as u64)
        });
        self.events.push(PubSubEvent::PublishQueuedFlushed { topic: topic.clone(), count: flushed });
      }
    }

    self.events.push(PubSubEvent::PartitionRecovered { topic: topic.clone() });
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
    let topic_snapshot: Vec<(PubSubTopic, PubSubTopicMetrics)> =
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

  /// Returns all known topics.
  #[must_use]
  pub fn topics(&self) -> Vec<PubSubTopic> {
    self.topics.keys().cloned().collect()
  }

  fn handle_partitioned_publish(
    &mut self,
    topic: &PubSubTopic,
    options: PubSubTopicOptions,
  ) -> Result<Vec<PubSubSubscriber<TB>>, PublishRejectReason> {
    let policy = options.delivery_policy;
    let behavior = options.partition_behavior;
    if matches!((policy, behavior), (DeliveryPolicy::AtLeastOnce, PartitionBehavior::DelayQueue))
      && let Some(entry) = self.topics.get_mut(topic)
    {
      entry.queued_message_count = entry.queued_message_count.saturating_add(1);
    }

    match (policy, behavior) {
      | (DeliveryPolicy::AtMostOnce, _) | (_, PartitionBehavior::Drop) => {
        self.metrics.dropped_messages = self.metrics.dropped_messages.saturating_add(1);
        self.bump_topic_metrics(topic, |m| m.dropped_messages = m.dropped_messages.saturating_add(1));
        self.events.push(PubSubEvent::PublishDroppedDueToPartition { topic: topic.clone() });
        Err(PublishRejectReason::PartitionDrop)
      },
      | (DeliveryPolicy::AtLeastOnce, PartitionBehavior::DelayQueue) => {
        self.metrics.delayed_messages = self.metrics.delayed_messages.saturating_add(1);
        self.bump_topic_metrics(topic, |m| m.delayed_messages = m.delayed_messages.saturating_add(1));
        self.events.push(PubSubEvent::PublishQueuedDueToPartition { topic: topic.clone() });
        Ok(Vec::new())
      },
    }
  }

  fn bump_topic_metrics<F>(&mut self, topic: &PubSubTopic, mut f: F)
  where
    F: FnMut(&mut PubSubTopicMetrics), {
    let entry = self.topic_metrics.entry(topic.clone()).or_default();
    f(entry);
  }
}

fn ttl_ticks(ttl: Duration, resolution: Duration) -> u64 {
  if resolution.is_zero() {
    return 0;
  }
  let ttl_nanos = ttl.as_nanos();
  let resolution_nanos = resolution.as_nanos();
  if resolution_nanos == 0 {
    return 0;
  }
  let ticks = ttl_nanos.div_ceil(resolution_nanos);
  ticks.min(u64::MAX as u128) as u64
}
