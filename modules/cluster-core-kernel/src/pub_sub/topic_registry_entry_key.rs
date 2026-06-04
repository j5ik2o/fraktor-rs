//! Key namespace for pub-sub registry entries.

use alloc::string::String;

use super::{MediatorPathKey, PubSubSubscriber, PubSubTopic};

/// Stable registry key for path and topic subscription entries.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TopicRegistryEntryKey {
  /// Actor path registration key.
  Path {
    /// Canonical address-less path key.
    path:   MediatorPathKey,
    /// Registered target.
    target: PubSubSubscriber,
  },
  /// Topic subscription key.
  TopicSubscription {
    /// Topic name.
    topic:      PubSubTopic,
    /// Optional subscriber group.
    group:      Option<String>,
    /// Subscriber target.
    subscriber: PubSubSubscriber,
  },
}
