#[cfg(test)]
#[path = "stream_ref_endpoint_slot_test.rs"]
mod tests;

use alloc::string::String;

use fraktor_actor_core_kernel_rs::actor::actor_ref::ActorRef;
use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};

use crate::StreamError;

/// Shared endpoint actor reference installed during StreamRef materialization.
#[derive(Clone)]
pub(crate) struct StreamRefEndpointSlot {
  actor_ref: ArcShared<SpinSyncMutex<Option<ActorRef>>>,
}

impl StreamRefEndpointSlot {
  /// Creates an empty endpoint slot.
  #[must_use]
  pub(crate) fn new() -> Self {
    Self { actor_ref: ArcShared::new(SpinSyncMutex::new(None)) }
  }

  /// Creates a slot that already points at an endpoint actor reference.
  #[must_use]
  pub(crate) fn from_actor_ref(actor_ref: ActorRef) -> Self {
    Self { actor_ref: ArcShared::new(SpinSyncMutex::new(Some(actor_ref))) }
  }

  /// Installs the materialized endpoint actor reference.
  pub(crate) fn set_actor_ref(&self, actor_ref: ActorRef) {
    *self.actor_ref.lock() = Some(actor_ref);
  }

  /// Returns the installed endpoint actor reference.
  pub(crate) fn actor_ref(&self) -> Result<ActorRef, StreamError> {
    self.actor_ref.lock().clone().ok_or(StreamError::StreamRefTargetNotInitialized)
  }

  /// Returns the canonical actor path used by resolver serialization support.
  pub(crate) fn canonical_actor_path(&self) -> Result<String, StreamError> {
    let actor_ref = self.actor_ref()?;
    actor_ref.canonical_path().map(|path| path.to_canonical_uri()).ok_or(StreamError::StreamRefTargetNotInitialized)
  }
}
