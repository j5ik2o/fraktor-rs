//! Adapter that turns a persistent actor into a runtime actor.

#[cfg(test)]
mod tests;

use alloc::{format, string::ToString};

use fraktor_actor_rs::core::{
  actor::{Actor, ActorContextGeneric},
  error::ActorError,
  messaging::AnyMessageViewGeneric,
};
use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::SharedAccess};

use crate::core::{
  journal_response::JournalResponse, persistence_extension_shared::PersistenceExtensionSharedGeneric,
  persistent_actor::PersistentActor, snapshot_response::SnapshotResponse,
};

/// Actor adapter that drives a persistent actor lifecycle.
pub(crate) struct PersistentActorAdapter<A, TB: RuntimeToolbox + 'static> {
  actor:   A,
  _marker: core::marker::PhantomData<TB>,
}

impl<A, TB> PersistentActorAdapter<A, TB>
where
  TB: RuntimeToolbox + 'static,
{
  /// Creates a new adapter around the provided persistent actor.
  #[must_use]
  pub(crate) const fn new(actor: A) -> Self {
    Self { actor, _marker: core::marker::PhantomData }
  }
}

impl<A, TB> Actor<TB> for PersistentActorAdapter<A, TB>
where
  A: PersistentActor<TB> + Sync + 'static,
  TB: RuntimeToolbox + 'static,
{
  fn pre_start(&mut self, ctx: &mut ActorContextGeneric<'_, TB>) -> Result<(), ActorError> {
    let extension = ctx
      .system()
      .extended()
      .extension_by_type::<PersistenceExtensionSharedGeneric<TB>>()
      .ok_or_else(|| ActorError::fatal("persistence extension not registered"))?;
    let (journal_actor_ref, snapshot_actor_ref) =
      extension.with_read(|ext| (ext.journal_actor_ref(), ext.snapshot_actor_ref()));
    let persistence_id = self.actor.persistence_id().to_string();
    let recovery = self.actor.recovery();
    let persistence_context = self.actor.persistence_context();
    if persistence_context.persistence_id() != persistence_id {
      return Err(ActorError::fatal("persistence_id mismatch"));
    }
    persistence_context
      .bind_actor_refs(journal_actor_ref, snapshot_actor_ref)
      .map_err(|error| ActorError::fatal(format!("{error:?}")))?;
    persistence_context.start_recovery(recovery, ctx.self_ref());
    Ok(())
  }

  fn receive(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    message: AnyMessageViewGeneric<'_, TB>,
  ) -> Result<(), ActorError> {
    if let Some(response) = message.downcast_ref::<JournalResponse>() {
      self.actor.handle_journal_response(response);
      return Ok(());
    }
    if let Some(response) = message.downcast_ref::<SnapshotResponse>() {
      self.actor.handle_snapshot_response(response, ctx);
      return Ok(());
    }
    self.actor.handle_command(ctx, message)
  }
}
