//! Bounded pub-sub registry delta payload.

use alloc::vec::Vec;

use crate::pub_sub::TopicRegistryDeltaEntry;

/// Delta payload containing registry entries newer than a peer status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicRegistryDelta {
  entries: Vec<TopicRegistryDeltaEntry>,
}

impl TopicRegistryDelta {
  /// Creates a delta from already bounded entries.
  #[must_use]
  pub(crate) const fn new(entries: Vec<TopicRegistryDeltaEntry>) -> Self {
    Self { entries }
  }

  /// Returns entries in version order.
  #[must_use]
  pub fn entries(&self) -> &[TopicRegistryDeltaEntry] {
    &self.entries
  }

  /// Returns the number of entries carried by this delta.
  #[must_use]
  pub const fn len(&self) -> usize {
    self.entries.len()
  }

  /// Returns true when this delta has no entries.
  #[must_use]
  pub const fn is_empty(&self) -> bool {
    self.entries.is_empty()
  }
}

impl Default for TopicRegistryDelta {
  fn default() -> Self {
    Self::new(Vec::new())
  }
}
