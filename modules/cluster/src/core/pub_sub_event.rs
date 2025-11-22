//! Events emitted by the pub/sub broker for EventStream/metrics.

use alloc::{string::String, vec::Vec};

use crate::core::pub_sub_topic_metrics::PubSubTopicMetrics;

/// Event kinds covering topic lifecycle and publishing outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PubSubEvent {
  /// New topic was created.
  TopicCreated {
    /// Topic name.
    topic: String,
  },
  /// Topic already existed when creation was requested.
  TopicAlreadyExists {
    /// Topic name.
    topic: String,
  },
  /// Subscription was accepted.
  SubscriptionAccepted {
    /// Topic name.
    topic:      String,
    /// Subscriber identifier.
    subscriber: String,
  },
  /// Subscription was rejected with a reason.
  SubscriptionRejected {
    /// Topic name.
    topic:      String,
    /// Subscriber identifier.
    subscriber: String,
    /// Rejection reason.
    reason:     String,
  },
  /// Publish failed because the topic does not exist.
  PublishRejectedMissingTopic {
    /// Topic name.
    topic: String,
  },
  /// Publish failed because there were no subscribers.
  PublishRejectedNoSubscribers {
    /// Topic name.
    topic: String,
  },
  /// Publish was queued because the topic is partitioned and allows delay.
  PublishQueuedDueToPartition {
    /// Topic name.
    topic: String,
  },
  /// Publish was dropped because the topic is partitioned and the policy forbids queueing.
  PublishDroppedDueToPartition {
    /// Topic name.
    topic: String,
  },
  /// Queued messages were flushed after partition recovery.
  PublishQueuedFlushed {
    /// Topic name.
    topic: String,
    /// Number of flushed messages.
    count: usize,
  },
  /// Partition state was marked.
  PartitionMarked {
    /// Topic name.
    topic: String,
  },
  /// Partition state was cleared.
  PartitionRecovered {
    /// Topic name.
    topic: String,
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
    snapshots: Vec<(String, PubSubTopicMetrics)>,
  },
}
