//! Query commands accepted by the distributed pub-sub mediator.

use super::PubSubTopic;

/// Query command for mediator registry snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediatorQuery {
  /// Returns all current topics.
  CurrentTopics,
  /// Returns subscriber count for a topic.
  SubscriberCount {
    /// Topic name.
    topic: PubSubTopic,
  },
}
