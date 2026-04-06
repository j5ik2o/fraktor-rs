//! Persistent actor state machine.

#[cfg(test)]
mod tests;

use crate::core::persistence_error::PersistenceError;

/// State machine for persistent actors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PersistentActorState {
  /// Waiting for recovery permit.
  WaitingRecoveryPermit,
  /// Recovery has started (snapshot load).
  RecoveryStarted,
  /// Replaying events.
  Recovering,
  /// Processing commands normally.
  ProcessingCommands,
  /// Persisting events and stashing commands.
  PersistingEvents,
}

impl PersistentActorState {
  /// Transitions to `RecoveryStarted`.
  ///
  /// # Errors
  ///
  /// Returns `PersistenceError::StateMachine` when the transition is invalid.
  pub fn transition_to_recovery_started(self) -> Result<Self, PersistenceError> {
    match self {
      | PersistentActorState::WaitingRecoveryPermit => Ok(PersistentActorState::RecoveryStarted),
      | _ => Err(PersistenceError::StateMachine("invalid transition to RecoveryStarted".into())),
    }
  }

  /// Transitions to `Recovering`.
  ///
  /// # Errors
  ///
  /// Returns `PersistenceError::StateMachine` when the transition is invalid.
  pub fn transition_to_recovering(self) -> Result<Self, PersistenceError> {
    match self {
      | PersistentActorState::RecoveryStarted => Ok(PersistentActorState::Recovering),
      | _ => Err(PersistenceError::StateMachine("invalid transition to Recovering".into())),
    }
  }

  /// Transitions to `ProcessingCommands`.
  ///
  /// # Errors
  ///
  /// Returns `PersistenceError::StateMachine` when the transition is invalid.
  pub fn transition_to_processing_commands(self) -> Result<Self, PersistenceError> {
    match self {
      | PersistentActorState::RecoveryStarted
      | PersistentActorState::Recovering
      | PersistentActorState::PersistingEvents => Ok(PersistentActorState::ProcessingCommands),
      | _ => Err(PersistenceError::StateMachine("invalid transition to ProcessingCommands".into())),
    }
  }

  /// Transitions to `PersistingEvents`.
  ///
  /// # Errors
  ///
  /// Returns `PersistenceError::StateMachine` when the transition is invalid.
  pub fn transition_to_persisting_events(self) -> Result<Self, PersistenceError> {
    match self {
      | PersistentActorState::ProcessingCommands => Ok(PersistentActorState::PersistingEvents),
      | _ => Err(PersistenceError::StateMachine("invalid transition to PersistingEvents".into())),
    }
  }
}
