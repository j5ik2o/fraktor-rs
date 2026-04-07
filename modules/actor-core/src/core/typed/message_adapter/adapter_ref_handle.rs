//! Adapter reference handle managed by the owning actor cell.

#[cfg(test)]
mod tests;

use fraktor_utils_core_rs::core::sync::ArcShared;

use crate::core::typed::message_adapter::{AdapterRefHandleId, adapter_lifecycle_state::AdapterLifecycleState};

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
