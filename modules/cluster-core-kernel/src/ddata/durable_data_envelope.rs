//! Wrapper for replicated data values persisted by a durable store.

use crate::ddata::ReplicatedData;

/// Serialization boundary for one durable distributed-data entry.
///
/// This mirrors Pekko's `DurableStore.DurableDataEnvelope`, which wraps the
/// replicated CRDT value carried by `Store` and `LoadData` messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DurableDataEnvelope<D: ReplicatedData> {
  data: D,
}

impl<D: ReplicatedData> DurableDataEnvelope<D> {
  /// Creates a durable envelope around the given CRDT value.
  #[must_use]
  pub const fn new(data: D) -> Self {
    Self { data }
  }

  /// Returns the wrapped CRDT value.
  #[must_use]
  pub const fn data(&self) -> &D {
    &self.data
  }

  /// Consumes the envelope and returns the wrapped CRDT value.
  #[must_use]
  pub fn into_data(self) -> D {
    self.data
  }
}
