//! Errors returned by pub/sub broker operations.

use alloc::string::String;

/// Errors that can occur while managing topics or publishing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PubSubError {
  /// Topic already exists.
  TopicAlreadyExists {
    /// Topic name.
    topic: String,
  },
  /// Topic is missing.
  TopicNotFound {
    /// Topic name.
    topic: String,
  },
  /// No subscribers are registered.
  NoSubscribers {
    /// Topic name.
    topic: String,
  },
  /// Subscriber is already registered to the topic.
  DuplicateSubscriber {
    /// Topic name.
    topic:      String,
    /// Subscriber identifier.
    subscriber: String,
  },
  /// Publish was dropped because the topic is partitioned and queueing is disabled.
  PartitionDrop {
    /// Topic name.
    topic: String,
  },
}
