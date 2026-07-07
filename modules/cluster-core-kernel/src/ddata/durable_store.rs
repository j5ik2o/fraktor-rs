//! Durable store port for distributed-data persistence.

#[cfg(test)]
#[path = "durable_store_test.rs"]
mod tests;

use super::{
  DurableStoreError, DurableStoreLoadAll, DurableStoreLoadAllCompleted, DurableStoreLoadData, DurableStoreStore,
  ReplicatedData,
};

/// Port backing durable persistence for the future Replicator runtime.
///
/// Implementations must follow the Pekko startup protocol:
/// respond to [`DurableStoreLoadAll`] with zero or more [`DurableStoreLoadData`] batches
/// followed by one [`DurableStoreLoadAllCompleted`], and handle [`DurableStoreStore`]
/// requests during normal operation.
pub trait DurableStore<D: ReplicatedData> {
  /// Persists one durable entry.
  ///
  /// # Errors
  ///
  /// Returns [`DurableStoreError::StoreFailed`] when the entry cannot be stored.
  fn store(&mut self, request: &DurableStoreStore<D>) -> Result<(), DurableStoreError>;

  /// Loads all durable entries for startup recovery.
  ///
  /// # Errors
  ///
  /// Returns [`DurableStoreError::LoadFailed`] when startup load cannot complete.
  fn load_all(&mut self) -> Result<DurableStoreLoadData<D>, DurableStoreError>;

  /// Marks startup load as completed.
  ///
  /// The default implementation is a no-op for stores that do not track load phase.
  fn complete_load_all(&mut self) -> Result<DurableStoreLoadAllCompleted, DurableStoreError> {
    Ok(DurableStoreLoadAllCompleted)
  }

  /// Executes the startup load protocol as one convenience operation.
  ///
  /// # Errors
  ///
  /// Returns [`DurableStoreError::LoadFailed`] when startup load cannot complete.
  fn startup_load(
    &mut self,
    _request: DurableStoreLoadAll,
  ) -> Result<(DurableStoreLoadData<D>, DurableStoreLoadAllCompleted), DurableStoreError> {
    let data = self.load_all()?;
    let completed = self.complete_load_all()?;
    Ok((data, completed))
  }
}
