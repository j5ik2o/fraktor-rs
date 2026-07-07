//! Durable store load-data response protocol message.

use alloc::{collections::BTreeMap, string::String};

use super::{DurableDataEnvelope, ReplicatedData};

/// Batch of durable entries returned during startup load.
///
/// This mirrors Pekko's `DurableStore.LoadData` message at the port level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DurableStoreLoadData<D: ReplicatedData> {
  data: BTreeMap<String, DurableDataEnvelope<D>>,
}

impl<D: ReplicatedData> DurableStoreLoadData<D> {
  /// Creates a load-data batch from the provided entries.
  #[must_use]
  pub fn new(data: BTreeMap<String, DurableDataEnvelope<D>>) -> Self {
    Self { data }
  }

  /// Returns the loaded entries.
  #[must_use]
  pub fn data(&self) -> &BTreeMap<String, DurableDataEnvelope<D>> {
    &self.data
  }

  /// Returns true when the batch contains no entries.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.data.is_empty()
  }
}
