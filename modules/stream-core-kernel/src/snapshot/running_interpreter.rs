#[cfg(test)]
mod tests;

use alloc::vec::Vec;

use crate::snapshot::{ConnectionSnapshot, InterpreterSnapshot, LogicSnapshot};

/// Snapshot of an interpreter that is currently running.
///
/// Corresponds to Pekko `RunningInterpreterImpl(logics, connections,
/// runningLogicsCount, stoppedLogics)` — a concrete `InterpreterSnapshot`
/// variant that carries the active stage logics, the connections between
/// them, the number of still-running logics and the logics that have already
/// terminated.
///
/// Pekko's `RunningInterpreterImpl` also carries a `queueStatus: String`
/// field marked as `HideImpl`. That field is an implementation detail that
/// is not part of the public `RunningInterpreter` contract, so it is not
/// modelled here.
#[derive(Debug, Clone)]
pub struct RunningInterpreter {
  logics:               Vec<LogicSnapshot>,
  connections:          Vec<ConnectionSnapshot>,
  running_logics_count: u32,
  stopped_logics:       Vec<LogicSnapshot>,
}

impl RunningInterpreter {
  /// Creates a new running-interpreter snapshot.
  #[must_use]
  pub const fn new(
    logics: Vec<LogicSnapshot>,
    connections: Vec<ConnectionSnapshot>,
    running_logics_count: u32,
    stopped_logics: Vec<LogicSnapshot>,
  ) -> Self {
    Self { logics, connections, running_logics_count, stopped_logics }
  }

  /// Returns the connections wired between the currently running logics.
  #[must_use]
  pub fn connections(&self) -> &[ConnectionSnapshot] {
    &self.connections
  }

  /// Returns the number of logics that are still running.
  #[must_use]
  pub const fn running_logics_count(&self) -> u32 {
    self.running_logics_count
  }

  /// Returns the logics that have already stopped.
  #[must_use]
  pub fn stopped_logics(&self) -> &[LogicSnapshot] {
    &self.stopped_logics
  }
}

impl InterpreterSnapshot for RunningInterpreter {
  fn logics(&self) -> &[LogicSnapshot] {
    &self.logics
  }
}
