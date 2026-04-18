#[cfg(test)]
mod tests;

use alloc::vec::Vec;

use crate::core::snapshot::{RunningInterpreter, UninitializedInterpreter};

/// Aggregated snapshot of a running stream.
///
/// Corresponds to Pekko `StreamSnapshotImpl(activeInterpreters, newShells)` —
/// a read-only view over the interpreters currently running in a materializer
/// and the new `GraphInterpreterShell`s awaiting initialisation.
#[derive(Debug, Clone)]
pub struct StreamSnapshot {
  active_interpreters: Vec<RunningInterpreter>,
  new_shells:          Vec<UninitializedInterpreter>,
}

impl StreamSnapshot {
  /// Creates a new stream snapshot.
  #[must_use]
  pub const fn new(active_interpreters: Vec<RunningInterpreter>, new_shells: Vec<UninitializedInterpreter>) -> Self {
    Self { active_interpreters, new_shells }
  }

  /// Returns the interpreters currently running in the materializer.
  #[must_use]
  pub fn active_interpreters(&self) -> &[RunningInterpreter] {
    &self.active_interpreters
  }

  /// Returns the interpreter shells awaiting initialisation.
  #[must_use]
  pub fn new_shells(&self) -> &[UninitializedInterpreter] {
    &self.new_shells
  }
}
