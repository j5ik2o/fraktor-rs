//! Persistent actor context and state.

#[cfg(test)]
mod tests;

use alloc::{boxed::Box, collections::VecDeque, format, string::String, vec::Vec};
use core::{
  ops::Deref,
  sync::atomic::{AtomicU32, Ordering},
};

use fraktor_actor_rs::core::{
  actor::{Pid, actor_ref::ActorRefGeneric},
  messaging::AnyMessageGeneric,
};
use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::ArcShared};

use crate::core::{
  event_adapters::EventAdapters, journal_message::JournalMessage, journal_response::JournalResponse,
  journal_response_action::JournalResponseAction, pending_handler_invocation::PendingHandlerInvocation,
  persistence_error::PersistenceError, persistent_actor_state::PersistentActorState,
  persistent_envelope::PersistentEnvelope, persistent_repr::PersistentRepr, recovery::Recovery,
  snapshot_error::SnapshotError, snapshot_message::SnapshotMessage, snapshot_response::SnapshotResponse,
  snapshot_response_action::SnapshotResponseAction,
};

type PendingHandler<A> = Box<dyn FnOnce(&mut A, &PersistentRepr) + Send + Sync>;

static NEXT_INSTANCE_ID: AtomicU32 = AtomicU32::new(1);

enum EventBatchEntry<A> {
  Persistent(PersistentEnvelope<A>),
  Deferred(Box<PendingHandlerInvocation<A>>),
}

/// Persistence context owned by persistent actors.
pub struct PersistenceContext<A: 'static, TB: RuntimeToolbox + 'static> {
  persistence_id: String,
  state: PersistentActorState,
  pending_invocations: VecDeque<PendingHandlerInvocation<A>>,
  stash_until_batch_completion: bool,
  event_batch: Vec<EventBatchEntry<A>>,
  current_sequence_nr: u64,
  last_sequence_nr: u64,
  recovery: Recovery,
  instance_id: u32,
  event_adapters: EventAdapters,
  journal_actor_ref: ActorRefGeneric<TB>,
  snapshot_actor_ref: ActorRefGeneric<TB>,
}

impl<A: 'static, TB: RuntimeToolbox + 'static> PersistenceContext<A, TB> {
  /// Creates a new persistence context for the provided persistence id.
  #[must_use]
  pub fn new(persistence_id: String) -> Self {
    Self {
      persistence_id,
      state: PersistentActorState::WaitingRecoveryPermit,
      pending_invocations: VecDeque::new(),
      stash_until_batch_completion: false,
      event_batch: Vec::new(),
      current_sequence_nr: 0,
      last_sequence_nr: 0,
      recovery: Recovery::default(),
      instance_id: NEXT_INSTANCE_ID.fetch_add(1, Ordering::Relaxed),
      event_adapters: EventAdapters::new(),
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

  /// Returns the event adapter registry.
  #[must_use]
  pub const fn event_adapters(&self) -> &EventAdapters {
    &self.event_adapters
  }

  /// Returns the mutable event adapter registry.
  pub const fn event_adapters_mut(&mut self) -> &mut EventAdapters {
    &mut self.event_adapters
  }

  /// Returns the current persistence instance id.
  #[must_use]
  pub(crate) const fn instance_id(&self) -> u32 {
    self.instance_id
  }

  /// Adds an event to the batch.
  pub fn add_to_event_batch<E: core::any::Any + Send + Sync + 'static>(
    &mut self,
    event: E,
    stashing: bool,
    sender: Option<Pid>,
    handler: PendingHandler<A>,
  ) {
    self.current_sequence_nr = self.current_sequence_nr.saturating_add(1);
    let envelope = PersistentEnvelope::new(ArcShared::new(event), self.current_sequence_nr, handler, stashing, sender);
    self.event_batch.push(EventBatchEntry::Persistent(envelope));
  }

  /// Adds a deferred handler invocation executed after successful batch persistence.
  pub fn add_deferred_handler<E: core::any::Any + Send + Sync + 'static>(
    &mut self,
    event: E,
    stashing: bool,
    sender: Option<Pid>,
    handler: PendingHandler<A>,
  ) {
    let repr = PersistentRepr::new(self.persistence_id.clone(), self.current_sequence_nr, ArcShared::new(event))
      .with_sender(sender)
      .with_adapters(self.event_adapters.clone());
    let invocation = if stashing {
      PendingHandlerInvocation::stashing_deferred_boxed(repr, handler)
    } else {
      PendingHandlerInvocation::async_deferred_boxed(repr, handler)
    };
    if self.event_batch.is_empty() {
      self.pending_invocations.push_back(invocation);
      return;
    }
    self.event_batch.push(EventBatchEntry::Deferred(Box::new(invocation)));
  }

  /// Returns true when there is in-flight persistence work.
  #[must_use]
  pub fn has_in_flight_persistence(&self) -> bool {
    self.state == PersistentActorState::PersistingEvents
      || !self.event_batch.is_empty()
      || !self.pending_invocations.is_empty()
  }

  /// Returns true when incoming commands should be stashed.
  #[must_use]
  pub fn should_stash_commands(&self) -> bool {
    let has_stashing_pending =
      self.pending_invocations.iter().any(|invocation| invocation.is_stashing() && !invocation.is_deferred());
    let has_stashing_deferred =
      self.pending_invocations.iter().any(|invocation| invocation.is_stashing() && invocation.is_deferred());
    (self.state == PersistentActorState::PersistingEvents
      && (has_stashing_pending || self.stash_until_batch_completion))
      || has_stashing_deferred
  }

  /// Flushes the current batch to the journal.
  ///
  /// # Errors
  ///
  /// Returns `PersistenceError` when the state transition or message send fails.
  /// On send failure the state is rolled back to `ProcessingCommands` and
  /// pending invocations are cleared.
  pub fn flush_batch(&mut self, sender: ActorRefGeneric<TB>) -> Result<(), PersistenceError> {
    if self.event_batch.is_empty() || !self.is_ready() {
      return Ok(());
    }

    let next_state = self.state.transition_to_persisting_events()?;
    self.state = next_state;

    let mut messages = Vec::new();
    let to_sequence_nr = self.current_sequence_nr;
    let mut has_stashing_invocation = false;

    for entry in self.event_batch.drain(..) {
      match entry {
        | EventBatchEntry::Persistent(envelope) => {
          let stashing = envelope.is_stashing();
          let repr = envelope.into_persistent_repr(self.persistence_id.clone(), self.event_adapters.clone());
          let journal_repr = Self::to_journal_repr(&repr);
          let handler = envelope.into_handler();
          let invocation = if stashing {
            has_stashing_invocation = true;
            PendingHandlerInvocation::stashing_boxed(repr.clone(), handler)
          } else {
            PendingHandlerInvocation::async_handler_boxed(repr.clone(), handler)
          };
          self.pending_invocations.push_back(invocation);
          messages.push(journal_repr);
        },
        | EventBatchEntry::Deferred(invocation) => {
          if invocation.is_stashing() {
            has_stashing_invocation = true;
          }
          self.pending_invocations.push_back(*invocation);
        },
      }
    }
    self.stash_until_batch_completion = has_stashing_invocation;

    debug_assert!(!messages.is_empty(), "flush_batch requires at least one persistent journal message");

    let message = JournalMessage::WriteMessages {
      persistence_id: self.persistence_id.clone(),
      to_sequence_nr,
      messages,
      sender,
      instance_id: self.instance_id,
    };
    if let Err(error) = self.send_write_messages(message) {
      // 送信失敗時: 状態をロールバックし、処理不能な保留ハンドラをクリア
      self.stash_until_batch_completion = false;
      self.pending_invocations.clear();
      if let Ok(rollback) = self.state.transition_to_processing_commands() {
        self.state = rollback;
      }
      return Err(error);
    }
    Ok(())
  }

  /// Handles journal responses.
  pub(crate) fn handle_journal_response(&mut self, response: &JournalResponse) -> JournalResponseAction<A> {
    match response {
      | JournalResponse::WriteMessageSuccess { repr, instance_id } => {
        if self.state != PersistentActorState::PersistingEvents || !self.matches_instance_id(*instance_id) {
          return JournalResponseAction::None;
        }
        self.last_sequence_nr = repr.sequence_nr();
        let action = Self::to_handler_action(self.take_invocations_for_write_success());
        self.transition_to_processing_commands_if_no_pending();
        action
      },
      | JournalResponse::WriteMessageFailure { repr, cause, instance_id } => {
        if !self.matches_instance_id(*instance_id) {
          return JournalResponseAction::None;
        }
        self.advance_after_write_failure(repr);
        JournalResponseAction::PersistFailure { cause: cause.clone(), repr: repr.clone() }
      },
      | JournalResponse::WriteMessageRejected { repr, cause, instance_id } => {
        if !self.matches_instance_id(*instance_id) {
          return JournalResponseAction::None;
        }
        self.advance_after_write_rejected(repr);
        JournalResponseAction::PersistRejected { cause: cause.clone(), repr: repr.clone() }
      },
      | JournalResponse::WriteMessagesFailed { write_count, instance_id, .. } => {
        if self.state != PersistentActorState::PersistingEvents || !self.matches_instance_id(*instance_id) {
          return JournalResponseAction::None;
        }
        if *write_count == 0 {
          self.reset_after_write_failure();
        }
        JournalResponseAction::None
      },
      | JournalResponse::WriteMessagesSuccessful { instance_id } => {
        if self.state != PersistentActorState::PersistingEvents || !self.matches_instance_id(*instance_id) {
          return JournalResponseAction::None;
        }
        self.stash_until_batch_completion = false;
        let action = Self::to_handler_action(self.take_leading_deferred_invocations());
        self.transition_to_processing_commands_if_no_pending();
        action
      },
      | JournalResponse::ReplayedMessage { persistent_repr } => {
        self.current_sequence_nr = persistent_repr.sequence_nr();
        let mut replayed_reprs = Self::from_journal_repr(persistent_repr);
        match replayed_reprs.len() {
          | 0 => JournalResponseAction::None,
          | 1 => {
            if let Some(replayed_repr) = replayed_reprs.pop() {
              JournalResponseAction::ReceiveRecover(replayed_repr)
            } else {
              JournalResponseAction::None
            }
          },
          | _ => JournalResponseAction::ReceiveRecoverMany(replayed_reprs),
        }
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
    if !self.is_ready() || self.state != PersistentActorState::RecoveryStarted {
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
        if let Err(error) = self.send_write_messages(message) {
          return SnapshotResponseAction::SnapshotFailure(SnapshotError::LoadFailed(format!(
            "failed to send replay messages: {error}"
          )));
        }
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
        if let Err(send_error) = self.send_write_messages(message) {
          return SnapshotResponseAction::SnapshotFailure(SnapshotError::LoadFailed(format!(
            "failed to send replay messages after snapshot load failure: {send_error}"
          )));
        }
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

  fn take_invocations_for_write_success(&mut self) -> Vec<PendingHandlerInvocation<A>> {
    let mut invocations = self.take_leading_deferred_invocations();
    if let Some(invocation) = self.pending_invocations.pop_front() {
      invocations.push(invocation);
    }
    invocations
  }

  fn take_leading_deferred_invocations(&mut self) -> Vec<PendingHandlerInvocation<A>> {
    let mut invocations = Vec::new();
    while self.pending_invocations.front().is_some_and(PendingHandlerInvocation::is_deferred) {
      if let Some(invocation) = self.pending_invocations.pop_front() {
        invocations.push(invocation);
      }
    }
    invocations
  }

  fn to_handler_action(mut invocations: Vec<PendingHandlerInvocation<A>>) -> JournalResponseAction<A> {
    match invocations.len() {
      | 0 => JournalResponseAction::None,
      | 1 => {
        let invocation = invocations.remove(0);
        JournalResponseAction::InvokeHandler(invocation)
      },
      | _ => JournalResponseAction::InvokeHandlers(invocations),
    }
  }

  fn transition_to_processing_commands_if_no_pending(&mut self) {
    if self.pending_invocations.is_empty()
      && !self.stash_until_batch_completion
      && let Ok(state) = self.state.transition_to_processing_commands()
    {
      self.state = state;
    }
  }

  fn advance_after_write_rejected(&mut self, repr: &PersistentRepr) {
    self.stash_until_batch_completion = false;
    self.remove_pending_persist_invocation(repr.sequence_nr());
    self.transition_to_processing_commands_if_no_pending();
  }

  fn advance_after_write_failure(&mut self, repr: &PersistentRepr) {
    self.remove_pending_persist_invocation(repr.sequence_nr());
  }

  fn remove_pending_persist_invocation(&mut self, sequence_nr: u64) {
    if let Some(index) = self
      .pending_invocations
      .iter()
      .position(|invocation| !invocation.is_deferred() && invocation.sequence_nr() == sequence_nr)
    {
      let _ = self.pending_invocations.remove(index);
    }
  }

  fn reset_after_write_failure(&mut self) {
    self.stash_until_batch_completion = false;
    self.pending_invocations.clear();
    self.transition_to_processing_commands_if_no_pending();
  }

  const fn matches_instance_id(&self, instance_id: u32) -> bool {
    self.instance_id == instance_id
  }

  /// Starts recovery.
  ///
  /// # Errors
  ///
  /// Returns `PersistenceError` when the state transition or message send fails.
  /// On send failure the state is rolled back to `WaitingRecoveryPermit`.
  pub(crate) fn start_recovery(
    &mut self,
    recovery: Recovery,
    sender: ActorRefGeneric<TB>,
  ) -> Result<(), PersistenceError> {
    self.ensure_ready()?;
    self.recovery = recovery;
    let next_state = self.state.transition_to_recovery_started()?;
    self.state = next_state;
    if self.recovery == Recovery::none() {
      let message = JournalMessage::GetHighestSequenceNr {
        persistence_id: self.persistence_id.clone(),
        from_sequence_nr: 0,
        sender,
      };
      if let Err(error) = self.send_write_messages(message) {
        // 送信失敗時: 状態をロールバック
        self.state = PersistentActorState::WaitingRecoveryPermit;
        return Err(error);
      }
      return Ok(());
    }

    let message = SnapshotMessage::LoadSnapshot {
      persistence_id: self.persistence_id.clone(),
      criteria: self.recovery.snapshot_criteria().clone(),
      sender,
    };
    if let Err(error) = self.send_snapshot_message(message) {
      // 送信失敗時: 状態をロールバック
      self.state = PersistentActorState::WaitingRecoveryPermit;
      return Err(error);
    }
    Ok(())
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

  fn to_journal_repr(repr: &PersistentRepr) -> PersistentRepr {
    let payload = repr.payload().clone();
    let adapter = repr.adapters().write_adapter_for_type_id(repr.adapter_type_id());
    let manifest = adapter.manifest(payload.deref());
    let adapted_payload = adapter.to_journal(payload);
    Self::repr_with_payload(repr, adapted_payload).with_manifest(manifest).with_sender(None)
  }

  fn from_journal_repr(repr: &PersistentRepr) -> Vec<PersistentRepr> {
    let payload = repr.payload().clone();
    let adapted =
      repr.adapters().read_adapter_for_type_id(repr.adapter_type_id()).adapt_from_journal(payload, repr.manifest());
    adapted.into_events().into_iter().map(|adapted_payload| Self::repr_with_payload(repr, adapted_payload)).collect()
  }

  fn repr_with_payload(repr: &PersistentRepr, payload: ArcShared<dyn core::any::Any + Send + Sync>) -> PersistentRepr {
    let updated = PersistentRepr::new(repr.persistence_id(), repr.sequence_nr(), payload)
      .with_manifest(repr.manifest())
      .with_writer_uuid(repr.writer_uuid())
      .with_timestamp(repr.timestamp())
      .with_deleted(repr.deleted())
      .with_sender(repr.sender())
      .with_adapters(repr.adapters().clone())
      .with_adapter_type_id(repr.adapter_type_id());
    if let Some(metadata) = repr.metadata() {
      return updated.with_metadata(metadata.clone());
    }
    updated
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
