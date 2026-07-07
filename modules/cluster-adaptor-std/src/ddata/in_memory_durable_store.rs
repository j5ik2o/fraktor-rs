//! In-memory durable store for tests and local development.

#[cfg(test)]
#[path = "in_memory_durable_store_test.rs"]
mod tests;

use alloc::{collections::BTreeMap, string::String};

use fraktor_cluster_core_kernel_rs::ddata::{
  DurableDataEnvelope, DurableStore, DurableStoreError, DurableStoreLoadData, DurableStoreStore, ReplicatedData,
};

/// Simple in-memory [`DurableStore`] implementation backed by a `BTreeMap`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InMemoryDurableStore<D: ReplicatedData> {
  entries: BTreeMap<String, DurableDataEnvelope<D>>,
}

impl<D: ReplicatedData> InMemoryDurableStore<D> {
  /// Creates an empty in-memory durable store.
  #[must_use]
  pub const fn new() -> Self {
    Self { entries: BTreeMap::new() }
  }

  /// Returns the number of persisted entries.
  #[must_use]
  pub fn len(&self) -> usize {
    self.entries.len()
  }

  /// Returns true when no entries are persisted.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.entries.is_empty()
  }
}

impl<D: ReplicatedData> DurableStore<D> for InMemoryDurableStore<D> {
  fn store(&mut self, request: &DurableStoreStore<D>) -> Result<(), DurableStoreError> {
    self.entries.insert(request.key().to_string(), request.data().clone());
    Ok(())
  }

  fn load_all(&mut self) -> Result<DurableStoreLoadData<D>, DurableStoreError> {
    Ok(DurableStoreLoadData::new(self.entries.clone()))
  }
}
