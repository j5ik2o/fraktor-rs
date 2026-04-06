//! Errors returned by pub/sub broker operations.

use alloc::string::String;

use super::PubSubTopic;

/// Errors that can occur while managing topics or publishing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PubSubError {
  /// Pub/Sub subsystem is not started.
  NotStarted,
  /// Topic already exists.
  TopicAlreadyExists {
    /// Topic name.
    topic: PubSubTopic,
  },
  /// Topic is missing.
  TopicNotFound {
    /// Topic name.
    topic: PubSubTopic,
  },
  /// Subscriber is already registered to the topic.
  DuplicateSubscriber {
    /// Topic name.
    topic:      PubSubTopic,
    /// Subscriber identifier.
    subscriber: String,
  },
  /// Subscriber is missing.
  SubscriberNotFound {
    /// Topic name.
    topic:      PubSubTopic,
    /// Subscriber identifier.
    subscriber: String,
  },
  /// Serialization failed unexpectedly.
  SerializationFailed {
    /// Failure reason.
    reason: String,
  },
  /// Delivery failed unexpectedly.
  DeliveryFailed {
    /// Failure reason.
    reason: String,
  },
}
