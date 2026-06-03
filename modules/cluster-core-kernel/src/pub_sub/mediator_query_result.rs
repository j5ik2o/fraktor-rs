//! Query result returned by the distributed pub-sub mediator.

use alloc::vec::Vec;

use super::PubSubTopic;

/// Completed query result for mediator registry snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediatorQueryResult {
  /// Current topic names.
  CurrentTopics {
    /// Topics observed in the registry snapshot.
    topics: Vec<PubSubTopic>,
  },
  /// Subscriber count for one topic.
  SubscriberCount {
    /// Topic name.
    topic: PubSubTopic,
    /// Number of subscribers currently registered for the topic.
    count: usize,
  },
}

impl MediatorQueryResult {
  /// Returns true when the query result is complete.
  #[must_use]
  pub const fn is_completed(&self) -> bool {
    true
  }
}
