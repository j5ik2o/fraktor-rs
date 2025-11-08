//! Adapter registry maintained per typed actor.

#[cfg(test)]
mod tests;

use alloc::{string::String, vec::Vec};
use core::any::TypeId;

use cellactor_utils_core_rs::sync::ArcShared;

use crate::{
  RuntimeToolbox,
  actor_prim::{ActorContextGeneric, Pid, actor_ref::ActorRefGeneric},
  system::SystemStateGeneric,
  typed::message_adapter::{
    AdapterEntry, AdapterError, AdapterFailure, AdapterOutcome, AdapterPayload, AdapterRefHandleId, AdapterRefSender,
  },
};

const MAX_ADAPTERS: usize = 32;

/// Stores adapter entries registered by a typed actor instance.
pub struct MessageAdapterRegistry<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static, {
  entries:        Vec<AdapterEntry<M, TB>>,
  adapter_ref:    Option<ActorRefGeneric<TB>>,
  adapter_handle: Option<AdapterRefHandleId>,
  system:         Option<ArcShared<SystemStateGeneric<TB>>>,
  pid:            Option<Pid>,
}

impl<M, TB> MessageAdapterRegistry<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static,
{
  /// Creates an empty registry.
  #[must_use]
  pub const fn new() -> Self {
    Self {
      entries:        Vec::new(),
      adapter_ref:    None,
      adapter_handle: None,
      system:         None,
      pid:            None,
    }
  }

  /// Returns the number of registered adapters.
  #[must_use]
  pub const fn len(&self) -> usize {
    self.entries.len()
  }

  /// Returns whether the registry contains no adapters.
  #[must_use]
  pub const fn is_empty(&self) -> bool {
    self.entries.is_empty()
  }

  /// Registers or replaces an adapter for the specified payload type.
  ///
  /// # Errors
  ///
  /// Returns [`AdapterError::RegistryFull`] when the registry reached its capacity or
  /// [`AdapterError::ActorUnavailable`] when the owning actor cell cannot be located.
  pub fn register<U, F>(
    &mut self,
    ctx: &ActorContextGeneric<'_, TB>,
    adapter: F,
  ) -> Result<ActorRefGeneric<TB>, AdapterError>
  where
    U: Send + Sync + 'static,
    F: Fn(U) -> Result<M, AdapterFailure> + Send + Sync + 'static, {
    let adapter_ref = self.ensure_adapter_ref(ctx)?;
    let type_id = TypeId::of::<U>();
    if let Some(position) = self.entries.iter().position(|entry: &AdapterEntry<M, TB>| entry.type_id() == type_id) {
      self.entries.remove(position);
    } else if self.entries.len() >= MAX_ADAPTERS {
      return Err(AdapterError::RegistryFull);
    }
    let entry = AdapterEntry::<M, TB>::new::<U, F>(type_id, adapter);
    self.entries.push(entry);
    Ok(adapter_ref)
  }

  /// Clears all registered adapters.
  pub fn clear(&mut self) {
    self.entries.clear();
    if let (Some(system), Some(pid), Some(handle_id)) = (&self.system, self.pid, self.adapter_handle.take())
      && let Some(cell) = system.cell(&pid)
    {
      cell.remove_adapter_handle(handle_id);
    }
    self.adapter_ref = None;
  }

  /// Attempts to adapt the provided payload.
  #[must_use]
  pub fn adapt(&self, payload: AdapterPayload<TB>) -> (AdapterOutcome<M>, Option<AdapterPayload<TB>>) {
    let payload_type = payload.type_id();
    let mut envelope = Some(payload);
    for entry in self.entries.iter().rev() {
      if entry.type_id() == payload_type {
        if let Some(concrete) = envelope.take() {
          return (entry.invoke(concrete), None);
        }
        return (AdapterOutcome::Failure(AdapterFailure::Custom(String::from("adapter_payload_consumed"))), None);
      }
    }
    (AdapterOutcome::NotFound, envelope)
  }

  fn ensure_adapter_ref(&mut self, ctx: &ActorContextGeneric<'_, TB>) -> Result<ActorRefGeneric<TB>, AdapterError> {
    if let Some(reference) = &self.adapter_ref {
      return Ok(reference.clone());
    }

    let system_state = ctx.system().state();
    let pid = ctx.pid();
    let cell = system_state.cell(&pid).ok_or(AdapterError::ActorUnavailable)?;
    let (handle_id, lifecycle) = cell.acquire_adapter_handle();
    let target = cell.mailbox_sender();
    let target_trait: ArcShared<dyn crate::actor_prim::actor_ref::ActorRefSender<TB>> = target;
    let adapter_sender =
      ArcShared::new(AdapterRefSender::new(pid, handle_id, target_trait, lifecycle, system_state.clone()));
    let adapter_ref = ActorRefGeneric::with_system(pid, adapter_sender, system_state.clone());

    self.adapter_ref = Some(adapter_ref.clone());
    self.adapter_handle = Some(handle_id);
    self.system = Some(system_state);
    self.pid = Some(pid);

    Ok(adapter_ref)
  }
}

impl<M, TB> Default for MessageAdapterRegistry<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + 'static,
{
  fn default() -> Self {
    Self::new()
  }
}
