//! Pub-sub registry payload carried by the gossip substrate.

use crate::pub_sub::{TopicRegistryDelta, TopicRegistryStatus};

/// Core pub-sub registry payload passed to gossip without owning envelope framing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TopicRegistryGossipPayload {
  /// Registry owner-version status.
  Status(TopicRegistryStatus),
  /// Bounded registry delta.
  Delta(TopicRegistryDelta),
}
