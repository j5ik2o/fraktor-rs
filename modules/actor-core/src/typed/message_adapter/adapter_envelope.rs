//! Envelope carrying adapter payloads through the mailbox.

#[cfg(test)]
mod tests;

use core::any::TypeId;

use fraktor_utils_core_rs::core::{runtime_toolbox::SyncMutexFamily, sync::sync_mutex_like::SyncMutexLike};

use crate::{
  RuntimeToolbox, ToolboxMutex, actor_prim::actor_ref::ActorRefGeneric, typed::message_adapter::AdapterPayload,
};

/// Wraps adapted payloads alongside metadata for typed actors.
pub struct AdapterEnvelope<TB: RuntimeToolbox + 'static> {
  type_id:  TypeId,
  payload:  ToolboxMutex<Option<AdapterPayload<TB>>, TB>,
  reply_to: Option<ActorRefGeneric<TB>>,
}

impl<TB: RuntimeToolbox + 'static> AdapterEnvelope<TB> {
  /// Creates a new envelope from the provided payload and reply target.
  #[must_use]
  pub fn new(payload: AdapterPayload<TB>, reply_to: Option<ActorRefGeneric<TB>>) -> Self {
    let type_id = payload.type_id();
    let storage = <TB::MutexFamily as SyncMutexFamily>::create(Some(payload));
    Self { type_id, payload: storage, reply_to }
  }

  /// Returns the [`TypeId`] of the underlying adapter payload.
  #[must_use]
  pub const fn type_id(&self) -> TypeId {
    self.type_id
  }

  /// Returns the reply target.
  #[must_use]
  pub const fn reply_to(&self) -> Option<&ActorRefGeneric<TB>> {
    self.reply_to.as_ref()
  }

  /// Takes ownership of the payload, returning `None` if it was already consumed.
  #[must_use]
  pub fn take_payload(&self) -> Option<AdapterPayload<TB>> {
    self.payload.lock().take()
  }

  /// Consumes the envelope and returns the payload if still available.
  #[must_use]
  pub fn into_payload(self) -> Option<AdapterPayload<TB>> {
    self.payload.into_inner()
  }
}
