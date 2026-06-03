//! Pub-sub registry payload handoff to logical gossip.

#[cfg(test)]
#[path = "pub_sub_gossip_handoff_test.rs"]
mod tests;

use crate::{
  membership::GossipPayloadKind,
  pub_sub::{TopicRegistryDelta, TopicRegistryGossipPayload, TopicRegistryStatus},
};

/// Payload and logical gossip kind produced by pub-sub registry gossip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PubSubGossipHandoff {
  payload_kind: GossipPayloadKind,
  payload:      TopicRegistryGossipPayload,
}

impl PubSubGossipHandoff {
  /// Creates a status handoff.
  #[must_use]
  pub const fn status(status: TopicRegistryStatus) -> Self {
    Self {
      payload_kind: GossipPayloadKind::PubSubRegistryStatus,
      payload:      TopicRegistryGossipPayload::Status(status),
    }
  }

  /// Creates a delta handoff.
  #[must_use]
  pub const fn delta(delta: TopicRegistryDelta) -> Self {
    Self {
      payload_kind: GossipPayloadKind::PubSubRegistryDelta,
      payload:      TopicRegistryGossipPayload::Delta(delta),
    }
  }

  /// Returns the logical gossip payload kind.
  #[must_use]
  pub const fn payload_kind(&self) -> GossipPayloadKind {
    self.payload_kind
  }

  /// Returns the pub-sub payload.
  #[must_use]
  pub const fn payload(&self) -> &TopicRegistryGossipPayload {
    &self.payload
  }
}
