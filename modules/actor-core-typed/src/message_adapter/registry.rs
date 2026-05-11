//! Adapter registry maintained per typed actor.

#[cfg(test)]
#[path = "registry_test.rs"]
mod tests;

use alloc::{string::String, vec::Vec};
use core::any::{Any, TypeId};

use fraktor_actor_core_kernel_rs::actor::{
  ActorContext, actor_ref::ActorRef, message_adapter::MessageAdapterLease, messaging::AnyMessage,
};
use fraktor_utils_core_rs::sync::ArcShared;

use crate::message_adapter::{AdapterEntry, AdapterEnvelope, AdapterError, AdapterOutcome, AdapterPayload};

const MAX_ADAPTERS: usize = 32;

/// Stores adapter entries registered by a typed actor instance.
pub struct MessageAdapterRegistry<M>
where
  M: Send + Sync + 'static, {
  entries:     Vec<AdapterEntry<M>>,
  adapter_ref: Option<ActorRef>,
  lease:       Option<MessageAdapterLease>,
}

impl<M> MessageAdapterRegistry<M>
where
  M: Send + Sync + 'static,
{
  /// Creates an empty registry.
  #[must_use]
  pub const fn new() -> Self {
    Self { entries: Vec::new(), adapter_ref: None, lease: None }
  }

  /// Returns the number of registered adapters.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)]
  pub fn len(&self) -> usize {
    self.entries.len()
  }

  /// Returns whether the registry contains no adapters.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)]
  pub fn is_empty(&self) -> bool {
    self.entries.is_empty()
  }

  /// Registers or replaces an adapter for the specified payload type.
  ///
  /// # Errors
  ///
  /// Returns [`AdapterError::RegistryFull`] when the registry reached its capacity or
  /// [`AdapterError::ActorUnavailable`] when the owning actor cell cannot be located.
  pub fn register<U, F>(&mut self, ctx: &ActorContext<'_>, adapter: F) -> Result<ActorRef, AdapterError>
  where
    U: Send + Sync + 'static,
    F: Fn(U) -> Result<M, AdapterError> + Send + Sync + 'static, {
    let adapter_ref = self.ensure_adapter_ref(ctx)?;
    let type_id = TypeId::of::<U>();
    if let Some(position) = self.entries.iter().position(|entry: &AdapterEntry<M>| entry.type_id() == type_id) {
      self.entries.remove(position);
    } else if self.entries.len() >= MAX_ADAPTERS {
      return Err(AdapterError::RegistryFull);
    }
    let entry = AdapterEntry::<M>::new::<U, F>(type_id, adapter);
    self.entries.push(entry);
    Ok(adapter_ref)
  }

  /// Clears all registered adapters.
  pub fn clear(&mut self) {
    self.entries.clear();
    if let Some(lease) = self.lease.take() {
      lease.release();
    }
    self.adapter_ref = None;
  }

  /// Attempts to adapt the provided payload.
  #[must_use]
  pub(crate) fn adapt(&self, payload: AdapterPayload) -> (AdapterOutcome<M>, Option<AdapterPayload>) {
    let payload_type = payload.type_id();
    let mut envelope = Some(payload);
    for entry in self.entries.iter().rev() {
      if entry.type_id() == payload_type {
        if let Some(concrete) = envelope.take() {
          return (entry.invoke(concrete), None);
        }
        return (AdapterOutcome::Failure(AdapterError::Custom(String::from("adapter_payload_consumed"))), None);
      }
    }
    (AdapterOutcome::NotFound, envelope)
  }

  fn ensure_adapter_ref(&mut self, ctx: &ActorContext<'_>) -> Result<ActorRef, AdapterError> {
    if let Some(reference) = &self.adapter_ref {
      return Ok(reference.clone());
    }

    let message_adapter_ref =
      ctx.create_message_adapter_ref(wrap_adapter_message).map_err(|_error| AdapterError::ActorUnavailable)?;
    let (adapter_ref, lease) = message_adapter_ref.into_parts();

    self.adapter_ref = Some(adapter_ref.clone());
    self.lease = Some(lease);

    Ok(adapter_ref)
  }
}

fn wrap_adapter_message(message: AnyMessage) -> AnyMessage {
  let (erased, sender, is_control, not_influence_receive_timeout) = message.into_parts();
  let payload = AdapterPayload::from_erased(erased);
  let envelope = AdapterEnvelope::new(payload, sender);
  let envelope_payload: ArcShared<dyn Any + Send + Sync + 'static> = ArcShared::new(envelope);
  AnyMessage::from_parts(envelope_payload, None, is_control, not_influence_receive_timeout)
}

impl<M> Default for MessageAdapterRegistry<M>
where
  M: Send + Sync + 'static,
{
  fn default() -> Self {
    Self::new()
  }
}
