//! Pub-sub registry entry with its mutation version.

use super::{TopicRegistryEntryKind, TopicRegistryVersion};

/// Versioned entry stored in a pub-sub registry bucket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicRegistryEntry {
  version: TopicRegistryVersion,
  kind:    TopicRegistryEntryKind,
}

impl TopicRegistryEntry {
  /// Creates a versioned registry entry.
  #[must_use]
  pub const fn new(version: TopicRegistryVersion, kind: TopicRegistryEntryKind) -> Self {
    Self { version, kind }
  }

  /// Returns this entry version.
  #[must_use]
  pub const fn version(&self) -> TopicRegistryVersion {
    self.version
  }

  /// Returns this entry value.
  #[must_use]
  pub const fn kind(&self) -> &TopicRegistryEntryKind {
    &self.kind
  }
}
