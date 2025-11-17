//! Adapter reference handle managed by the owning actor cell.

#[cfg(test)]
mod tests;

use fraktor_utils_core_rs::core::sync::ArcShared;

use crate::{
  RuntimeToolbox,
  typed::message_adapter::{adapter_lifecycle_state::AdapterLifecycleState, adapter_ref_handle_id::AdapterRefHandleId},
};

/// Registered adapter handle storing lifecycle information.
#[derive(Clone)]
pub struct AdapterRefHandle<TB: RuntimeToolbox + 'static> {
  id:        AdapterRefHandleId,
  lifecycle: ArcShared<AdapterLifecycleState<TB>>,
}

impl<TB: RuntimeToolbox + 'static> AdapterRefHandle<TB> {
  /// Creates a new handle.
  #[must_use]
  pub const fn new(id: AdapterRefHandleId, lifecycle: ArcShared<AdapterLifecycleState<TB>>) -> Self {
    Self { id, lifecycle }
  }

  /// Returns the handle identifier.
  #[must_use]
  pub const fn id(&self) -> AdapterRefHandleId {
    self.id
  }

  /// Marks the associated lifecycle as stopped.
  pub fn stop(&self) {
    self.lifecycle.mark_stopped();
  }

  /// Returns a clone of the lifecycle state.
  #[must_use]
  pub fn lifecycle(&self) -> ArcShared<AdapterLifecycleState<TB>> {
    self.lifecycle.clone()
  }
}
