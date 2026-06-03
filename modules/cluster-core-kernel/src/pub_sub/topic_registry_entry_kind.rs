//! Value namespace for pub-sub registry entries.

use alloc::string::String;

use super::{MediatorPathKey, PubSubSubscriber, PubSubTopic};

/// Registry entry value stored in a topic registry bucket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TopicRegistryEntryKind {
  /// Actor path registration.
  Path {
    /// Canonical address-less path key.
    path:   MediatorPathKey,
    /// Registered target.
    target: PubSubSubscriber,
  },
  /// Topic subscription registration.
  TopicSubscription {
    /// Topic name.
    topic:      PubSubTopic,
    /// Optional subscriber group.
    group:      Option<String>,
    /// Subscriber target.
    subscriber: PubSubSubscriber,
  },
  /// Removed tombstone retained for gossip convergence.
  Removed {
    /// Millisecond timestamp supplied by the mediator caller.
    removed_at_millis: u64,
  },
}

impl TopicRegistryEntryKind {
  /// Returns true when this entry is a removed tombstone.
  #[must_use]
  pub const fn is_removed(&self) -> bool {
    matches!(self, Self::Removed { .. })
  }
}
