//! Adapter reference handle managed by the owning actor cell.

#[cfg(test)]
#[path = "adapter_ref_handle_test.rs"]
mod tests;

use fraktor_utils_core_rs::sync::ArcShared;

use super::{AdapterLifecycleState, AdapterRefHandleId};

/// Registered adapter handle storing lifecycle information.
#[derive(Clone)]
pub(crate) struct AdapterRefHandle {
  id:        AdapterRefHandleId,
  lifecycle: ArcShared<AdapterLifecycleState>,
}

impl AdapterRefHandle {
  /// Creates a new handle.
  #[must_use]
  pub(crate) const fn new(id: AdapterRefHandleId, lifecycle: ArcShared<AdapterLifecycleState>) -> Self {
    Self { id, lifecycle }
  }

  /// Returns the handle identifier.
  #[must_use]
  pub(crate) const fn id(&self) -> AdapterRefHandleId {
    self.id
  }

  /// Marks the associated lifecycle as stopped.
  pub(crate) fn stop(&self) {
    self.lifecycle.mark_stopped();
  }
}
