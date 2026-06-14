//! Query result returned by the distributed pub-sub mediator.

use alloc::vec::Vec;

use super::PubSubTopic;

/// Completed query result for mediator registry snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediatorQueryResult {
  /// Total subscriber registrations across all current topics.
  Count {
    /// Number of subscriber registrations currently observed by the mediator.
    count: usize,
  },
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
