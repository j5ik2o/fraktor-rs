//! Envelope carrying adapter payloads through the mailbox.

#[cfg(test)]
mod tests;

use core::any::TypeId;

use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, ToolboxMutex, sync_mutex_family::SyncMutexFamily},
  sync::sync_mutex_like::SyncMutexLike,
};

use crate::core::{actor::actor_ref::ActorRefGeneric, typed::message_adapter::AdapterPayload};

/// Wraps adapted payloads alongside metadata for typed actors.
pub(crate) struct AdapterEnvelope<TB: RuntimeToolbox + 'static> {
  type_id: TypeId,
  payload: ToolboxMutex<Option<AdapterPayload<TB>>, TB>,
  sender:  Option<ActorRefGeneric<TB>>,
}

impl<TB: RuntimeToolbox + 'static> AdapterEnvelope<TB> {
  /// Creates a new envelope from the provided payload and sender.
  #[must_use]
  pub(crate) fn new(payload: AdapterPayload<TB>, sender: Option<ActorRefGeneric<TB>>) -> Self {
    let type_id = payload.type_id();
    let storage = <TB::MutexFamily as SyncMutexFamily>::create(Some(payload));
    Self { type_id, payload: storage, sender }
  }

  /// Returns the [`TypeId`] of the underlying adapter payload.
  #[must_use]
  pub(crate) const fn type_id(&self) -> TypeId {
    self.type_id
  }

  /// Returns the sender.
  #[must_use]
  pub(crate) const fn sender(&self) -> Option<&ActorRefGeneric<TB>> {
    self.sender.as_ref()
  }

  /// Takes ownership of the payload, returning `None` if it was already consumed.
  #[must_use]
  pub(crate) fn take_payload(&self) -> Option<AdapterPayload<TB>> {
    self.payload.lock().take()
  }
}
