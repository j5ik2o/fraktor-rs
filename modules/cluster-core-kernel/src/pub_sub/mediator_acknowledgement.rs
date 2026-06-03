//! Acknowledgement returned by mediator subscription commands.

use alloc::string::String;

use super::{PubSubSubscriber, PubSubTopic};

/// Successful acknowledgement emitted by mediator subscription commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediatorAcknowledgement {
  /// Subscribe command completed.
  SubscribeCompleted {
    /// Subscribed topic.
    topic:      PubSubTopic,
    /// Optional subscriber group.
    group:      Option<String>,
    /// Subscriber target.
    subscriber: PubSubSubscriber,
  },
  /// Unsubscribe command completed.
  UnsubscribeCompleted {
    /// Unsubscribed topic.
    topic:      PubSubTopic,
    /// Optional subscriber group.
    group:      Option<String>,
    /// Subscriber target.
    subscriber: PubSubSubscriber,
  },
}

impl MediatorAcknowledgement {
  /// Returns true when the acknowledgement represents a completed operation.
  #[must_use]
  pub const fn is_completed(&self) -> bool {
    true
  }
}
