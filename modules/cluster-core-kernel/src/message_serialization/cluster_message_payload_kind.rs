//! Stable cluster message payload kind tags.

#[cfg(test)]
#[path = "cluster_message_payload_kind_test.rs"]
mod tests;

/// Stable protocol-family tag carried by cluster serialized messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClusterMessagePayloadKind {
  /// Gossip envelope payload.
  Gossip,
  /// Publish-subscribe mediator payload.
  PubSub,
}

impl ClusterMessagePayloadKind {
  /// Stable wire tag for gossip payloads.
  pub const GOSSIP_TAG: u16 = 1;
  /// Stable wire tag for publish-subscribe payloads.
  pub const PUB_SUB_TAG: u16 = 2;

  /// Returns the stable payload kind tag.
  #[must_use]
  pub const fn tag(self) -> u16 {
    match self {
      | Self::Gossip => Self::GOSSIP_TAG,
      | Self::PubSub => Self::PUB_SUB_TAG,
    }
  }

  /// Decodes a stable payload kind tag.
  #[must_use]
  pub const fn from_tag(tag: u16) -> Option<Self> {
    match tag {
      | Self::GOSSIP_TAG => Some(Self::Gossip),
      | Self::PUB_SUB_TAG => Some(Self::PubSub),
      | _ => None,
    }
  }
}
