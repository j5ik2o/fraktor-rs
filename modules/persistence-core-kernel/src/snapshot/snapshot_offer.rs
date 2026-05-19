//! Snapshot offer signal.

#[cfg(test)]
#[path = "snapshot_offer_test.rs"]
mod tests;

use core::any::Any;

use crate::snapshot::{Snapshot, SnapshotMetadata};

/// Snapshot offered to a persistent actor during recovery.
#[derive(Clone, Debug)]
pub struct SnapshotOffer {
  snapshot: Snapshot,
}

impl SnapshotOffer {
  /// Creates a new snapshot offer.
  #[must_use]
  pub const fn new(snapshot: Snapshot) -> Self {
    Self { snapshot }
  }

  /// Returns the offered snapshot.
  #[must_use]
  pub const fn snapshot(&self) -> &Snapshot {
    &self.snapshot
  }

  /// Returns the snapshot metadata.
  #[must_use]
  pub const fn metadata(&self) -> &SnapshotMetadata {
    self.snapshot.metadata()
  }

  /// Attempts to downcast the snapshot payload.
  #[must_use]
  pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
    self.snapshot.downcast_ref::<T>()
  }
}
