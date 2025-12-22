//! Events emitted by the pub/sub subsystem for EventStream/metrics.

use alloc::{string::String, vec::Vec};

use crate::core::{DeliveryStatus, PubSubTopic, PubSubTopicMetrics, PublishRejectReason};

/// Event kinds covering topic lifecycle and publishing outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PubSubEvent {
  /// New topic was created.
  TopicCreated {
    /// Topic name.
    topic: PubSubTopic,
  },
  /// Topic already existed when creation was requested.
  TopicAlreadyExists {
    /// Topic name.
    topic: PubSubTopic,
  },
  /// Subscription was accepted.
  SubscriptionAdded {
    /// Topic name.
    topic:      PubSubTopic,
    /// Subscriber identifier.
    subscriber: String,
  },
  /// Subscription was removed.
  SubscriptionRemoved {
    /// Topic name.
    topic:      PubSubTopic,
    /// Subscriber identifier.
    subscriber: String,
    /// Removal reason.
    reason:     String,
  },
  /// Subscription was rejected with a reason.
  SubscriptionRejected {
    /// Topic name.
    topic:      PubSubTopic,
    /// Subscriber identifier.
    subscriber: String,
    /// Rejection reason.
    reason:     String,
  },
  /// Publish accepted and delivery started.
  PublishAccepted {
    /// Topic name.
    topic:            PubSubTopic,
    /// Number of delivery targets.
    subscriber_count: usize,
  },
  /// Publish rejected with a reason.
  PublishRejected {
    /// Topic name.
    topic:  PubSubTopic,
    /// Rejection reason.
    reason: PublishRejectReason,
  },
  /// Publish was queued because the topic is partitioned and allows delay.
  PublishQueuedDueToPartition {
    /// Topic name.
    topic: PubSubTopic,
  },
  /// Publish was dropped because the topic is partitioned and the policy forbids queueing.
  PublishDroppedDueToPartition {
    /// Topic name.
    topic: PubSubTopic,
  },
  /// Queued messages were flushed after partition recovery.
  PublishQueuedFlushed {
    /// Topic name.
    topic: PubSubTopic,
    /// Number of flushed messages.
    count: usize,
  },
  /// Partition state was marked.
  PartitionMarked {
    /// Topic name.
    topic: PubSubTopic,
  },
  /// Partition state was cleared.
  PartitionRecovered {
    /// Topic name.
    topic: PubSubTopic,
  },
  /// Delivery succeeded for a subscriber.
  DeliverySucceeded {
    /// Topic name.
    topic:      PubSubTopic,
    /// Subscriber identifier.
    subscriber: String,
  },
  /// Delivery failed for a subscriber.
  DeliveryFailed {
    /// Topic name.
    topic:      PubSubTopic,
    /// Subscriber identifier.
    subscriber: String,
    /// Failure classification.
    status:     DeliveryStatus,
  },
  /// Metrics snapshot emitted for observability pipeline.
  MetricsSnapshot {
    /// Delayed messages queued during partition.
    delayed_messages:     u64,
    /// Dropped messages due to policy or missing subscribers.
    dropped_messages:     u64,
    /// Messages flushed after recovery.
    redelivered_messages: u64,
  },
  /// Metrics snapshot per topic for dashboard consumption.
  MetricsSnapshotByTopic {
    /// Topic-level snapshots.
    snapshots: Vec<(PubSubTopic, PubSubTopicMetrics)>,
  },
}
