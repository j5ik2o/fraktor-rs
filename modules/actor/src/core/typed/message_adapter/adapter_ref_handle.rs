//! Adapter reference handle managed by the owning actor cell.

#[cfg(test)]
mod tests;

use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::ArcShared};

use crate::core::typed::message_adapter::{
  adapter_lifecycle_state::AdapterLifecycleState, adapter_ref_handle_id::AdapterRefHandleId,
};

/// Registered adapter handle storing lifecycle information.
#[derive(Clone)]
pub(crate) struct AdapterRefHandle<TB: RuntimeToolbox + 'static> {
  id:        AdapterRefHandleId,
  lifecycle: ArcShared<AdapterLifecycleState<TB>>,
}

impl<TB: RuntimeToolbox + 'static> AdapterRefHandle<TB> {
  /// Creates a new handle.
  #[must_use]
  pub(crate) const fn new(id: AdapterRefHandleId, lifecycle: ArcShared<AdapterLifecycleState<TB>>) -> Self {
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
