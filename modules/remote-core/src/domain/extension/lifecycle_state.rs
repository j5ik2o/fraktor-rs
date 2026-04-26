//! Closed `&mut self` lifecycle state machine for the remote subsystem.

use crate::domain::extension::remoting_error::RemotingError;

/// Internal lifecycle phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
  /// Fresh instance; `start` has not been invoked yet.
  Pending,
  /// `transition_to_start` has been called but startup is still in progress.
  Starting,
  /// Startup completed successfully.
  Running,
  /// `transition_to_shutdown` has been called but shutdown is still in flight.
  ShuttingDown,
  /// Terminal state.
  Shutdown,
}

/// Closed lifecycle state machine for the remote subsystem.
///
/// The five phases (`Pending` → `Starting` → `Running` → `ShuttingDown` →
/// `Shutdown`) form a strict DAG with the single shortcut
/// `Pending → Shutdown` (graceful terminate before any startup was
/// attempted). Every transition is `&mut self`; invalid transitions return
/// a [`RemotingError`] without mutating the state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemotingLifecycleState {
  phase: Phase,
}

impl RemotingLifecycleState {
  /// Creates a new state machine in the `Pending` phase.
  #[must_use]
  pub const fn new() -> Self {
    Self { phase: Phase::Pending }
  }

  /// Attempts to move from `Pending` to `Starting`.
  ///
  /// # Errors
  ///
  /// Returns [`RemotingError::AlreadyRunning`] if the state is already
  /// `Starting` or `Running`, or [`RemotingError::InvalidTransition`] if
  /// the state is terminal (`ShuttingDown` / `Shutdown`).
  pub const fn transition_to_start(&mut self) -> Result<(), RemotingError> {
    match self.phase {
      | Phase::Pending => {
        self.phase = Phase::Starting;
        Ok(())
      },
      | Phase::Starting | Phase::Running => Err(RemotingError::AlreadyRunning),
      | Phase::ShuttingDown | Phase::Shutdown => Err(RemotingError::InvalidTransition),
    }
  }

  /// Moves from `Starting` to `Running`.
  ///
  /// # Errors
  ///
  /// Returns [`RemotingError::InvalidTransition`] from any state other
  /// than `Starting`.
  pub const fn mark_started(&mut self) -> Result<(), RemotingError> {
    match self.phase {
      | Phase::Starting => {
        self.phase = Phase::Running;
        Ok(())
      },
      | _ => Err(RemotingError::InvalidTransition),
    }
  }

  /// Rolls a failed startup attempt back from `Starting` to `Pending`.
  ///
  /// # Errors
  ///
  /// Returns [`RemotingError::InvalidTransition`] from any state other
  /// than `Starting`.
  pub const fn mark_start_failed(&mut self) -> Result<(), RemotingError> {
    match self.phase {
      | Phase::Starting => {
        self.phase = Phase::Pending;
        Ok(())
      },
      | _ => Err(RemotingError::InvalidTransition),
    }
  }

  /// Moves out of the live states towards termination:
  ///
  /// - `Running` → `ShuttingDown`
  /// - `Pending` → `Shutdown` (graceful terminate without ever running)
  ///
  /// # Errors
  ///
  /// Returns [`RemotingError::InvalidTransition`] from `Starting` /
  /// `ShuttingDown` / `Shutdown` states, which either cannot meaningfully
  /// transition to shutdown (`Starting`) or are already terminating.
  pub const fn transition_to_shutdown(&mut self) -> Result<(), RemotingError> {
    match self.phase {
      | Phase::Running => {
        self.phase = Phase::ShuttingDown;
        Ok(())
      },
      | Phase::Pending => {
        self.phase = Phase::Shutdown;
        Ok(())
      },
      | Phase::Starting | Phase::ShuttingDown | Phase::Shutdown => Err(RemotingError::InvalidTransition),
    }
  }

  /// Moves from `ShuttingDown` to `Shutdown`.
  ///
  /// # Errors
  ///
  /// Returns [`RemotingError::InvalidTransition`] from any state other
  /// than `ShuttingDown`.
  pub const fn mark_shutdown(&mut self) -> Result<(), RemotingError> {
    match self.phase {
      | Phase::ShuttingDown => {
        self.phase = Phase::Shutdown;
        Ok(())
      },
      | _ => Err(RemotingError::InvalidTransition),
    }
  }

  /// Returns `true` only when the state is exactly the `Running` phase.
  #[must_use]
  pub const fn is_running(&self) -> bool {
    matches!(self.phase, Phase::Running)
  }

  /// Returns `true` when the state is terminal (`Shutdown`).
  #[must_use]
  pub const fn is_terminated(&self) -> bool {
    matches!(self.phase, Phase::Shutdown)
  }

  /// Asserts that the lifecycle is in the `Running` state.
  ///
  /// # Errors
  ///
  /// Returns [`RemotingError::NotStarted`] from any non-`Running` state.
  pub const fn ensure_running(&self) -> Result<(), RemotingError> {
    if self.is_running() { Ok(()) } else { Err(RemotingError::NotStarted) }
  }
}

impl Default for RemotingLifecycleState {
  fn default() -> Self {
    Self::new()
  }
}
