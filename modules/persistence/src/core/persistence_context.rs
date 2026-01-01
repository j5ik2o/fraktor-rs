//! Persistent actor context and state.

#[cfg(test)]
mod tests;

use alloc::{boxed::Box, collections::VecDeque, format, string::String, vec::Vec};

use fraktor_actor_rs::core::{
  actor::{Pid, actor_ref::ActorRefGeneric},
  messaging::AnyMessageGeneric,
};
use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::ArcShared};

use crate::core::{
  journal_message::JournalMessage, journal_response::JournalResponse, journal_response_action::JournalResponseAction,
  pending_handler_invocation::PendingHandlerInvocation, persistence_error::PersistenceError,
  persistent_actor_state::PersistentActorState, persistent_envelope::PersistentEnvelope,
  persistent_repr::PersistentRepr, recovery::Recovery, snapshot_message::SnapshotMessage,
  snapshot_response::SnapshotResponse, snapshot_response_action::SnapshotResponseAction,
};

type PendingHandler<A> = Box<dyn FnOnce(&mut A, &PersistentRepr) + Send + Sync>;

/// Persistence context owned by persistent actors.
pub struct PersistenceContext<A: 'static, TB: RuntimeToolbox + 'static> {
  persistence_id:      String,
  state:               PersistentActorState,
  pending_invocations: VecDeque<PendingHandlerInvocation<A>>,
  event_batch:         Vec<PersistentEnvelope<A>>,
  journal_batch:       Vec<PersistentEnvelope<A>>,
  current_sequence_nr: u64,
  last_sequence_nr:    u64,
  recovery:            Recovery,
  instance_id:         u32,
  journal_actor_ref:   ActorRefGeneric<TB>,
  snapshot_actor_ref:  ActorRefGeneric<TB>,
}

impl<A: 'static, TB: RuntimeToolbox + 'static> PersistenceContext<A, TB> {
  /// Creates a new persistence context for the provided persistence id.
  #[must_use]
  pub fn new(persistence_id: String) -> Self {
    Self {
      persistence_id,
      state: PersistentActorState::WaitingRecoveryPermit,
      pending_invocations: VecDeque::new(),
      event_batch: Vec::new(),
      journal_batch: Vec::new(),
      current_sequence_nr: 0,
      last_sequence_nr: 0,
      recovery: Recovery::default(),
      instance_id: 1,
      journal_actor_ref: ActorRefGeneric::null(),
      snapshot_actor_ref: ActorRefGeneric::null(),
    }
  }

  /// Binds journal and snapshot actor references.
  ///
  /// # Errors
  ///
  /// Returns `PersistenceError::StateMachine` when called more than once or refs are invalid.
  pub fn bind_actor_refs(
    &mut self,
    journal_actor_ref: ActorRefGeneric<TB>,
    snapshot_actor_ref: ActorRefGeneric<TB>,
  ) -> Result<(), PersistenceError> {
    if self.is_bound() {
      return Err(PersistenceError::StateMachine("persistence actor refs already bound".into()));
    }
    if Self::is_null_ref(&journal_actor_ref) || Self::is_null_ref(&snapshot_actor_ref) {
      return Err(PersistenceError::StateMachine("persistence actor refs must be bound to concrete actors".into()));
    }
    self.journal_actor_ref = journal_actor_ref;
    self.snapshot_actor_ref = snapshot_actor_ref;
    Ok(())
  }

  /// Returns the current state.
  #[must_use]
  pub const fn state(&self) -> PersistentActorState {
    self.state
  }

  /// Returns the persistence id.
  #[must_use]
  pub fn persistence_id(&self) -> &str {
    &self.persistence_id
  }

  /// Returns the current sequence number.
  #[must_use]
  pub const fn current_sequence_nr(&self) -> u64 {
    self.current_sequence_nr
  }

  /// Returns the last sequence number.
  #[must_use]
  pub const fn last_sequence_nr(&self) -> u64 {
    self.last_sequence_nr
  }

  /// Adds an event to the batch.
  pub fn add_to_event_batch<E: core::any::Any + Send + Sync + 'static>(
    &mut self,
    event: E,
    stashing: bool,
    handler: PendingHandler<A>,
  ) {
    self.current_sequence_nr = self.current_sequence_nr.saturating_add(1);
    let envelope = PersistentEnvelope::new(ArcShared::new(event), self.current_sequence_nr, handler, stashing);
    self.event_batch.push(envelope);
  }

  /// Flushes the current batch to the journal.
  pub fn flush_batch(&mut self, sender: ActorRefGeneric<TB>) {
    if self.event_batch.is_empty() || !self.is_ready() {
      return;
    }

    self.journal_batch.append(&mut self.event_batch);
    let mut messages = Vec::new();

    for envelope in self.journal_batch.drain(..) {
      let stashing = envelope.is_stashing();
      let repr = envelope.into_persistent_repr(self.persistence_id.clone());
      let handler = envelope.into_handler();
      let invocation = if stashing {
        PendingHandlerInvocation::stashing(repr.clone(), handler)
      } else {
        PendingHandlerInvocation::async_handler(repr.clone(), handler)
      };
      self.pending_invocations.push_back(invocation);
      messages.push(repr);
    }

    let to_sequence_nr = messages.last().map(|repr| repr.sequence_nr()).unwrap_or(self.current_sequence_nr);
    let message = JournalMessage::WriteMessages {
      persistence_id: self.persistence_id.clone(),
      to_sequence_nr,
      messages,
      sender,
      instance_id: self.instance_id,
    };
    let _ = self.send_write_messages(message);
    if let Ok(state) = self.state.transition_to_persisting_events() {
      self.state = state;
    }
  }

  /// Handles journal responses.
  pub(crate) fn handle_journal_response(&mut self, response: &JournalResponse) -> JournalResponseAction<A> {
    match response {
      | JournalResponse::WriteMessageSuccess { repr, .. } => {
        let action = self
          .pending_invocations
          .pop_front()
          .map(JournalResponseAction::InvokeHandler)
          .unwrap_or(JournalResponseAction::None);
        self.last_sequence_nr = repr.sequence_nr();
        if self.pending_invocations.is_empty()
          && let Ok(state) = self.state.transition_to_processing_commands()
        {
          self.state = state;
        }
        action
      },
      | JournalResponse::WriteMessageFailure { repr, cause, .. } => {
        let _ = self.pending_invocations.pop_front();
        JournalResponseAction::PersistFailure { cause: cause.clone(), repr: repr.clone() }
      },
      | JournalResponse::WriteMessageRejected { repr, cause, .. } => {
        let _ = self.pending_invocations.pop_front();
        JournalResponseAction::PersistRejected { cause: cause.clone(), repr: repr.clone() }
      },
      | JournalResponse::ReplayedMessage { persistent_repr } => {
        self.current_sequence_nr = persistent_repr.sequence_nr();
        JournalResponseAction::ReceiveRecover(persistent_repr.clone())
      },
      | JournalResponse::RecoverySuccess { highest_sequence_nr } => {
        let highest = (*highest_sequence_nr).max(self.current_sequence_nr).max(self.last_sequence_nr);
        self.last_sequence_nr = highest;
        self.current_sequence_nr = highest;
        if let Ok(state) = self.state.transition_to_processing_commands() {
          self.state = state;
        }
        JournalResponseAction::RecoveryCompleted
      },
      | JournalResponse::HighestSequenceNr { sequence_nr, .. } => {
        self.last_sequence_nr = *sequence_nr;
        self.current_sequence_nr = *sequence_nr;
        if let Ok(state) = self.state.transition_to_processing_commands() {
          self.state = state;
        }
        JournalResponseAction::RecoveryCompleted
      },
      | JournalResponse::HighestSequenceNrFailure { cause, .. } => {
        JournalResponseAction::RecoveryFailure(PersistenceError::from(cause.clone()))
      },
      | JournalResponse::ReplayMessagesFailure { cause } => {
        JournalResponseAction::RecoveryFailure(PersistenceError::from(cause.clone()))
      },
      | JournalResponse::DeleteMessagesFailure { cause, .. } => {
        JournalResponseAction::RecoveryFailure(PersistenceError::from(cause.clone()))
      },
      | _ => JournalResponseAction::None,
    }
  }

  /// Handles snapshot responses.
  pub(crate) fn handle_snapshot_response(
    &mut self,
    response: &SnapshotResponse,
    sender: ActorRefGeneric<TB>,
  ) -> SnapshotResponseAction {
    if !self.is_ready() {
      return SnapshotResponseAction::None;
    }

    match response {
      | SnapshotResponse::LoadSnapshotResult { snapshot, .. } => {
        if let Some(snapshot) = snapshot {
          let sequence_nr = snapshot.metadata().sequence_nr();
          self.current_sequence_nr = sequence_nr;
          self.last_sequence_nr = sequence_nr;
        }
        if let Ok(state) = self.state.transition_to_recovering() {
          self.state = state;
        }
        let recovery = self.recovery.clone();
        let from_sequence_nr =
          snapshot.as_ref().map(|snap| snap.metadata().sequence_nr().saturating_add(1)).unwrap_or(0);
        let message = JournalMessage::ReplayMessages {
          persistence_id: self.persistence_id.clone(),
          from_sequence_nr,
          to_sequence_nr: recovery.to_sequence_nr(),
          max: recovery.replay_max(),
          sender,
        };
        let _ = self.send_write_messages(message);
        snapshot
          .as_ref()
          .map(|snap| SnapshotResponseAction::ReceiveSnapshot(snap.clone()))
          .unwrap_or(SnapshotResponseAction::None)
      },
      | SnapshotResponse::LoadSnapshotFailed { error } => {
        if let Ok(state) = self.state.transition_to_recovering() {
          self.state = state;
        }
        let recovery = self.recovery.clone();
        let message = JournalMessage::ReplayMessages {
          persistence_id: self.persistence_id.clone(),
          from_sequence_nr: 0,
          to_sequence_nr: recovery.to_sequence_nr(),
          max: recovery.replay_max(),
          sender,
        };
        let _ = self.send_write_messages(message);
        SnapshotResponseAction::SnapshotFailure(error.clone())
      },
      | SnapshotResponse::SaveSnapshotFailure { error, .. } => SnapshotResponseAction::SnapshotFailure(error.clone()),
      | SnapshotResponse::DeleteSnapshotFailure { error, .. } => SnapshotResponseAction::SnapshotFailure(error.clone()),
      | SnapshotResponse::DeleteSnapshotsFailure { error, .. } => {
        SnapshotResponseAction::SnapshotFailure(error.clone())
      },
      | _ => SnapshotResponseAction::None,
    }
  }

  /// Starts recovery.
  pub(crate) fn start_recovery(&mut self, recovery: Recovery, sender: ActorRefGeneric<TB>) {
    if !self.is_ready() {
      return;
    }
    self.recovery = recovery;
    if let Ok(state) = self.state.transition_to_recovery_started() {
      self.state = state;
    }
    let message = SnapshotMessage::LoadSnapshot {
      persistence_id: self.persistence_id.clone(),
      criteria: self.recovery.snapshot_criteria().clone(),
      sender,
    };
    let _ = self.send_snapshot_message(message);
  }

  /// Sends journal messages through the persistence extension.
  ///
  /// # Errors
  ///
  /// Returns `PersistenceError::StateMachine` when context is unbound.
  /// Returns `PersistenceError::MessagePassing` when the message cannot be delivered.
  pub fn send_write_messages(&self, message: JournalMessage<TB>) -> Result<(), PersistenceError> {
    self.ensure_ready()?;
    self
      .journal_actor_ref
      .tell(AnyMessageGeneric::new(message))
      .map_err(|error| PersistenceError::MessagePassing(format!("{error:?}")))
      .map(|_| ())
  }

  /// Sends snapshot messages through the persistence extension.
  ///
  /// # Errors
  ///
  /// Returns `PersistenceError::StateMachine` when context is unbound.
  /// Returns `PersistenceError::MessagePassing` when the message cannot be delivered.
  pub fn send_snapshot_message(&self, message: SnapshotMessage<TB>) -> Result<(), PersistenceError> {
    self.ensure_ready()?;
    self
      .snapshot_actor_ref
      .tell(AnyMessageGeneric::new(message))
      .map_err(|error| PersistenceError::MessagePassing(format!("{error:?}")))
      .map(|_| ())
  }

  fn ensure_ready(&self) -> Result<(), PersistenceError> {
    if !self.is_ready() {
      return Err(PersistenceError::StateMachine("persistence context not bound".into()));
    }
    Ok(())
  }

  fn is_bound(&self) -> bool {
    !Self::is_null_ref(&self.journal_actor_ref) || !Self::is_null_ref(&self.snapshot_actor_ref)
  }

  fn is_ready(&self) -> bool {
    !Self::is_null_ref(&self.journal_actor_ref) && !Self::is_null_ref(&self.snapshot_actor_ref)
  }

  fn is_null_ref(actor_ref: &ActorRefGeneric<TB>) -> bool {
    actor_ref.pid() == Pid::new(0, 0)
  }
}
