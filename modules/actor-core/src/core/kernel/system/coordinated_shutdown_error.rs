//! Errors from coordinated shutdown operations.

use alloc::string::String;
use core::fmt::{self, Formatter, Result as FmtResult};

/// Errors that can occur during coordinated shutdown operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoordinatedShutdownError {
  /// The specified phase is not defined in the phase graph.
  UnknownPhase(String),
  /// A cycle was detected in the phase dependency graph.
  CyclicDependency(String),
  /// The task name was empty.
  EmptyTaskName,
  /// The shutdown sequence has already been started.
  RunAlreadyStarted,
}

impl fmt::Display for CoordinatedShutdownError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    match self {
      | Self::UnknownPhase(phase) => write!(f, "unknown phase [{phase}]"),
      | Self::CyclicDependency(phase) => {
        write!(f, "cycle detected in phase graph: phase [{phase}] depends transitively on itself")
      },
      | Self::EmptyTaskName => write!(f, "task name must not be empty"),
      | Self::RunAlreadyStarted => write!(f, "shutdown has already been started"),
    }
  }
}
