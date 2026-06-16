//! Local entry state used by distributed-data protocol evaluation.

/// Local state for a distributed-data key before the Replicator runtime applies transport policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplicatorEntry<D> {
  /// No value has been observed for the key.
  Missing,
  /// A CRDT value is present for the key.
  Present(D),
  /// The key has been deleted and should reject later reads or updates.
  Deleted,
}

impl<D> ReplicatorEntry<D> {
  /// Returns a missing entry.
  #[must_use]
  pub const fn missing() -> Self {
    Self::Missing
  }

  /// Returns an entry containing `data`.
  #[must_use]
  pub const fn present(data: D) -> Self {
    Self::Present(data)
  }

  /// Returns a deleted entry.
  #[must_use]
  pub const fn deleted() -> Self {
    Self::Deleted
  }

  /// Returns true when the entry is missing.
  #[must_use]
  pub const fn is_missing(&self) -> bool {
    matches!(self, Self::Missing)
  }

  /// Returns true when the entry holds a data value.
  #[must_use]
  pub const fn is_present(&self) -> bool {
    matches!(self, Self::Present(_))
  }

  /// Returns true when the entry has been deleted.
  #[must_use]
  pub const fn is_deleted(&self) -> bool {
    matches!(self, Self::Deleted)
  }

  /// Returns the present data value, when available.
  #[must_use]
  pub const fn data(&self) -> Option<&D> {
    match self {
      | Self::Missing | Self::Deleted => None,
      | Self::Present(data) => Some(data),
    }
  }
}
