//! Adapter that turns a persistent actor into a runtime actor.

#[cfg(test)]
mod tests;

use alloc::{format, string::ToString};
use core::time::Duration;

use fraktor_actor_rs::core::{
  actor::{Actor, ActorContextGeneric},
  error::ActorError,
  messaging::{AnyMessageGeneric, AnyMessageViewGeneric},
  scheduler::{SchedulerCommand, SchedulerHandle},
};
use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::SharedAccess};

use crate::core::{
  journal_response::JournalResponse, persistence_error::PersistenceError,
  persistence_extension_shared::PersistenceExtensionSharedGeneric, persistent_actor::PersistentActor,
  persistent_actor_state::PersistentActorState, recovery_timed_out::RecoveryTimedOut,
  snapshot_response::SnapshotResponse, stash_overflow_strategy::StashOverflowStrategy,
};

#[derive(Clone, Copy)]
struct RecoveryTick {
  waiting_snapshot: bool,
  epoch:            u64,
}

impl RecoveryTick {
  const fn waiting_snapshot(epoch: u64) -> Self {
    Self { waiting_snapshot: true, epoch }
  }

  const fn waiting_event(epoch: u64) -> Self {
    Self { waiting_snapshot: false, epoch }
  }

  const fn is_waiting_snapshot(self) -> bool {
    self.waiting_snapshot
  }

  const fn epoch(self) -> u64 {
    self.epoch
  }
}

/// Actor adapter that drives a persistent actor lifecycle.
pub(crate) struct PersistentActorAdapter<A, TB: RuntimeToolbox + 'static> {
  actor:                   A,
  recovery_timeout_handle: Option<SchedulerHandle>,
  recovery_timeout_epoch:  u64,
  _marker:                 core::marker::PhantomData<TB>,
}

impl<A, TB> PersistentActorAdapter<A, TB>
where
  A: PersistentActor<TB> + 'static,
  TB: RuntimeToolbox + 'static,
{
  /// Creates a new adapter around the provided persistent actor.
  #[must_use]
  pub(crate) const fn new(actor: A) -> Self {
    Self { actor, recovery_timeout_handle: None, recovery_timeout_epoch: 0, _marker: core::marker::PhantomData }
  }

  fn cancel_recovery_timeout(&mut self, ctx: &ActorContextGeneric<'_, TB>) {
    if let Some(handle) = self.recovery_timeout_handle.take() {
      let scheduler = ctx.system().scheduler();
      scheduler.with_write(|guard| {
        guard.cancel(&handle);
      });
    }
  }

  fn schedule_recovery_timeout(
    &mut self,
    ctx: &ActorContextGeneric<'_, TB>,
    waiting_snapshot: bool,
  ) -> Result<(), ActorError> {
    self.cancel_recovery_timeout(ctx);
    let timeout = self.actor.recovery_event_timeout();
    if timeout == Duration::ZERO {
      return Err(ActorError::fatal("recovery_event_timeout must be greater than zero"));
    }
    self.recovery_timeout_epoch = self.recovery_timeout_epoch.wrapping_add(1);
    let tick = if waiting_snapshot {
      RecoveryTick::waiting_snapshot(self.recovery_timeout_epoch)
    } else {
      RecoveryTick::waiting_event(self.recovery_timeout_epoch)
    };
    let self_ref = ctx.self_ref();
    let scheduler = ctx.system().scheduler();
    let handle = scheduler
      .with_write(|guard| {
        guard.schedule_once(timeout, SchedulerCommand::SendMessage {
          receiver:   self_ref,
          message:    AnyMessageGeneric::new(tick),
          dispatcher: None,
          sender:     None,
        })
      })
      .map_err(|error| ActorError::fatal(format!("failed to schedule recovery timeout: {error}")))?;
    self.recovery_timeout_handle = Some(handle);
    Ok(())
  }

  fn handle_recovery_tick(&mut self, ctx: &ActorContextGeneric<'_, TB>, tick: RecoveryTick) -> Result<(), ActorError> {
    if tick.epoch() != self.recovery_timeout_epoch {
      return Ok(());
    }
    let state = self.actor.persistence_context().state();
    let timed_out = if tick.is_waiting_snapshot() {
      state == PersistentActorState::RecoveryStarted
    } else {
      state == PersistentActorState::Recovering
    };
    if !timed_out {
      return Ok(());
    }

    self.cancel_recovery_timeout(ctx);
    let signal = RecoveryTimedOut::new(self.actor.persistence_id().to_string());
    self.actor.on_recovery_timed_out(&signal);
    let timeout = self.actor.recovery_event_timeout();
    let reason = if tick.is_waiting_snapshot() {
      format!(
        "recovery timed out for persistence id {} while waiting for snapshot within {:?}",
        signal.persistence_id(),
        timeout
      )
    } else {
      format!(
        "recovery timed out for persistence id {} while waiting for event within {:?}, highest sequence number seen {}",
        signal.persistence_id(),
        timeout,
        self.actor.last_sequence_nr()
      )
    };
    self.actor.on_recovery_failure(&PersistenceError::Recovery(reason.clone()));
    Err(ActorError::fatal(reason))
  }

  fn update_recovery_timeout_after_snapshot_response(
    &mut self,
    ctx: &ActorContextGeneric<'_, TB>,
    response: &SnapshotResponse,
  ) -> Result<(), ActorError> {
    match response {
      | SnapshotResponse::LoadSnapshotResult { .. } | SnapshotResponse::LoadSnapshotFailed { .. } => {
        if self.actor.persistence_context().state() == PersistentActorState::Recovering {
          self.schedule_recovery_timeout(ctx, false)?;
        }
      },
      | _ => {},
    }
    Ok(())
  }

  fn update_recovery_timeout_after_journal_response(
    &mut self,
    ctx: &ActorContextGeneric<'_, TB>,
    response: &JournalResponse,
  ) -> Result<(), ActorError> {
    match response {
      | JournalResponse::ReplayedMessage { .. } => {
        if self.actor.persistence_context().state() == PersistentActorState::Recovering {
          self.schedule_recovery_timeout(ctx, false)?;
        }
      },
      | JournalResponse::RecoverySuccess { .. }
      | JournalResponse::ReplayMessagesFailure { .. }
      | JournalResponse::HighestSequenceNr { .. }
      | JournalResponse::HighestSequenceNrFailure { .. } => {
        self.cancel_recovery_timeout(ctx);
      },
      | _ => {},
    }
    Ok(())
  }

  fn stash_current_message(&self, ctx: &ActorContextGeneric<'_, TB>) -> Result<(), ActorError> {
    match ctx.stash_with_limit(self.actor.stash_capacity()) {
      | Ok(()) => Ok(()),
      | Err(error) if ActorContextGeneric::<TB>::is_stash_overflow_error(&error) => {
        match self.actor.stash_overflow_strategy() {
          | StashOverflowStrategy::Drop => Ok(()),
          | StashOverflowStrategy::Fail => Err(error),
        }
      },
      | Err(error) => Err(error),
    }
  }

  const fn is_current_instance_response(response: &JournalResponse, current_instance_id: u32) -> bool {
    match response {
      | JournalResponse::WriteMessageSuccess { instance_id, .. }
      | JournalResponse::WriteMessageFailure { instance_id, .. }
      | JournalResponse::WriteMessageRejected { instance_id, .. }
      | JournalResponse::WriteMessagesSuccessful { instance_id }
      | JournalResponse::WriteMessagesFailed { instance_id, .. } => *instance_id == current_instance_id,
      | _ => true,
    }
  }

  fn should_unstash_after_journal_response(&mut self, response: &JournalResponse, current_instance_id: u32) -> bool {
    Self::is_current_instance_response(response, current_instance_id)
      && matches!(
        response,
        JournalResponse::WriteMessagesSuccessful { .. }
          | JournalResponse::WriteMessageRejected { .. }
          | JournalResponse::WriteMessagesFailed { .. }
          | JournalResponse::RecoverySuccess { .. }
          | JournalResponse::HighestSequenceNr { .. }
      )
      && self.actor.persistence_context().state() == PersistentActorState::ProcessingCommands
  }

  fn is_recovery_running(&mut self) -> bool {
    matches!(
      self.actor.persistence_context().state(),
      PersistentActorState::RecoveryStarted | PersistentActorState::Recovering
    )
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
    persistence_context
      .start_recovery(recovery, ctx.self_ref())
      .map_err(|error| ActorError::fatal(format!("{error:?}")))?;
    self.schedule_recovery_timeout(ctx, true)?;
    Ok(())
  }

  fn receive(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    message: AnyMessageViewGeneric<'_, TB>,
  ) -> Result<(), ActorError> {
    if let Some(response) = message.downcast_ref::<JournalResponse>() {
      let current_instance_id = self.actor.persistence_context().instance_id();
      let recovery_running = self.is_recovery_running();
      self.actor.handle_journal_response(response);
      self.update_recovery_timeout_after_journal_response(ctx, response)?;
      if recovery_running {
        match response {
          | JournalResponse::ReplayMessagesFailure { cause } => {
            return Err(ActorError::fatal(format!(
              "persistent actor stopped after replay failure for persistence id {}: {:?}",
              self.actor.persistence_id(),
              cause
            )));
          },
          | JournalResponse::HighestSequenceNrFailure { persistence_id, cause } => {
            return Err(ActorError::fatal(format!(
              "persistent actor stopped after highest sequence number lookup failure for persistence id {}: {:?}",
              persistence_id, cause
            )));
          },
          | _ => {},
        }
      }
      if let JournalResponse::WriteMessageFailure { repr, cause, instance_id } = response
        && *instance_id == current_instance_id
      {
        return Err(ActorError::fatal(format!(
          "persistent actor stopped after write failure for persistence id {} sequence number {}: {:?}",
          repr.persistence_id(),
          repr.sequence_nr(),
          cause
        )));
      }
      if self.should_unstash_after_journal_response(response, current_instance_id) {
        let _ = ctx.unstash_all()?;
      }
      return Ok(());
    }
    if let Some(response) = message.downcast_ref::<SnapshotResponse>() {
      self.actor.handle_snapshot_response(response, ctx);
      self.update_recovery_timeout_after_snapshot_response(ctx, response)?;
      return Ok(());
    }
    if let Some(tick) = message.downcast_ref::<RecoveryTick>() {
      self.handle_recovery_tick(ctx, *tick)?;
      return Ok(());
    }
    if let Some(signal) = message.downcast_ref::<RecoveryTimedOut>() {
      self.actor.on_recovery_timed_out(signal);
      return Ok(());
    }
    let recovery_running = self.is_recovery_running();
    let should_stash = self.actor.persistence_context().should_stash_commands();
    if recovery_running || should_stash {
      return self.stash_current_message(ctx);
    }
    self.actor.handle_command(ctx, message)
  }

  fn post_stop(&mut self, ctx: &mut ActorContextGeneric<'_, TB>) -> Result<(), ActorError> {
    self.cancel_recovery_timeout(ctx);
    Ok(())
  }
}
