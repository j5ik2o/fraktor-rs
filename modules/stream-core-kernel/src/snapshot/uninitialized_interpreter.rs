#[cfg(test)]
mod tests;

use alloc::vec::Vec;

use crate::snapshot::{InterpreterSnapshot, LogicSnapshot};

/// Snapshot of an interpreter that has not yet been started.
///
/// Corresponds to Pekko `UninitializedInterpreterImpl(logics)` — a concrete
/// `InterpreterSnapshot` variant that carries only the pre-materialised
/// stage logics.
#[derive(Debug, Clone)]
pub struct UninitializedInterpreter {
  logics: Vec<LogicSnapshot>,
}

impl UninitializedInterpreter {
  /// Creates a new uninitialized-interpreter snapshot.
  #[must_use]
  pub const fn new(logics: Vec<LogicSnapshot>) -> Self {
    Self { logics }
  }
}

impl InterpreterSnapshot for UninitializedInterpreter {
  fn logics(&self) -> &[LogicSnapshot] {
    &self.logics
  }
}
