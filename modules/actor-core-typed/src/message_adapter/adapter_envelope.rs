//! Envelope carrying adapter payloads through the mailbox.

#[cfg(test)]
#[path = "adapter_envelope_test.rs"]
mod tests;

use core::any::TypeId;

use fraktor_actor_core_kernel_rs::actor::actor_ref::ActorRef;
use fraktor_utils_core_rs::sync::{DefaultMutex, SharedLock};

use crate::message_adapter::AdapterPayload;

/// Wraps adapted payloads alongside metadata for typed actors.
pub(crate) struct AdapterEnvelope {
  type_id: TypeId,
  payload: SharedLock<Option<AdapterPayload>>,
  sender:  Option<ActorRef>,
}

impl AdapterEnvelope {
  /// Creates a new envelope from the provided payload and sender.
  #[must_use]
  pub(crate) fn new(payload: AdapterPayload, sender: Option<ActorRef>) -> Self {
    let type_id = payload.type_id();
    let storage = SharedLock::new_with_driver::<DefaultMutex<_>>(Some(payload));
    Self { type_id, payload: storage, sender }
  }

  /// Returns the [`TypeId`] of the underlying adapter payload.
  #[must_use]
  pub(crate) const fn type_id(&self) -> TypeId {
    self.type_id
  }

  /// Returns the sender.
  #[must_use]
  pub(crate) const fn sender(&self) -> Option<&ActorRef> {
    self.sender.as_ref()
  }

  /// Takes ownership of the payload, returning `None` if it was already consumed.
  #[must_use]
  pub(crate) fn take_payload(&self) -> Option<AdapterPayload> {
    self.payload.with_lock(|payload| payload.take())
  }
}
