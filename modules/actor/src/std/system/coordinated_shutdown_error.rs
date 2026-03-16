//! Errors from coordinated shutdown operations.

extern crate std;

use alloc::string::String;
use core::fmt;

/// Errors that can occur during coordinated shutdown operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoordinatedShutdownError {
  /// The specified phase is not defined in the phase graph.
  UnknownPhase(String),
  /// A cycle was detected in the phase dependency graph.
  CyclicDependency(String),
  /// The task name was empty.
  EmptyTaskName,
}

impl fmt::Display for CoordinatedShutdownError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | Self::UnknownPhase(phase) => write!(f, "unknown phase [{phase}]"),
      | Self::CyclicDependency(phase) => {
        write!(f, "cycle detected in phase graph: phase [{phase}] depends transitively on itself")
      },
      | Self::EmptyTaskName => write!(f, "task name must not be empty"),
    }
  }
}

impl std::error::Error for CoordinatedShutdownError {}
