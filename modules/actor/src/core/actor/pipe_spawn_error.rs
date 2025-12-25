//! Errors that can occur while scheduling pipe-to-self tasks.

use core::fmt;

/// Describes failures encountered when spawning a pipe task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipeSpawnError {
  /// Indicates that the actor cell was unavailable (e.g., already stopped).
  ActorUnavailable,
  /// Indicates that the actor is no longer running.
  TargetStopped,
}

impl PipeSpawnError {
  /// Returns `true` if the target actor is no longer available.
  #[must_use]
  pub const fn is_target_stopped(&self) -> bool {
    matches!(self, Self::TargetStopped)
  }
}

impl fmt::Display for PipeSpawnError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | Self::ActorUnavailable => write!(f, "actor cell is unavailable"),
      | Self::TargetStopped => write!(f, "actor stopped before pipe task completed"),
    }
  }
}
