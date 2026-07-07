//! Function that modifies a CRDT value during an update command.

use alloc::{string::String, sync::Arc};

use fraktor_cluster_core_kernel_rs::ddata::ReplicatedData;

/// Function that modifies a CRDT value during an update command.
pub struct UpdateModifyFn<D: ReplicatedData + Send + Sync + 'static> {
  modify: Arc<dyn Fn(Option<&D>) -> Result<D, String> + Send + Sync>,
}

impl<D: ReplicatedData + Send + Sync + 'static> Clone for UpdateModifyFn<D> {
  fn clone(&self) -> Self {
    Self { modify: self.modify.clone() }
  }
}

impl<D: ReplicatedData + Send + Sync + 'static> UpdateModifyFn<D> {
  /// Creates a modify function wrapper.
  pub fn new<F>(modify: F) -> Self
  where
    F: Fn(Option<&D>) -> Result<D, String> + Send + Sync + 'static, {
    Self { modify: Arc::new(modify) }
  }

  /// Applies the modify function to the provided entry snapshot.
  ///
  /// # Errors
  ///
  /// Returns the modify failure message when the function rejects the update.
  pub fn apply(&self, entry: Option<&D>) -> Result<D, String> {
    (self.modify)(entry)
  }
}
