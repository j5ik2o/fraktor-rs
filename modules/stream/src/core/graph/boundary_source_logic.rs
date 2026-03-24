//! Source logic for the downstream side of an island boundary.
//!
//! `BoundarySourceLogic` implements `SourceLogic` and pulls elements from
//! a shared `IslandBoundaryShared`. When the buffer is empty and the upstream
//! has completed or failed, the corresponding signal is propagated.

use super::island_boundary::{BoundaryState, IslandBoundaryShared};
use crate::core::{DynValue, SourceLogic, StreamError};

#[cfg(test)]
mod tests;

/// Source stage logic that pulls elements from an inter-island boundary buffer.
pub(crate) struct BoundarySourceLogic {
  boundary: IslandBoundaryShared,
}

impl BoundarySourceLogic {
  /// Creates a new boundary source logic connected to the given shared boundary.
  #[must_use]
  pub(crate) const fn new(boundary: IslandBoundaryShared) -> Self {
    Self { boundary }
  }
}

impl SourceLogic for BoundarySourceLogic {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    let (value, state) = self.boundary.try_pull_with_state();
    if let Some(value) = value {
      return Ok(Some(value));
    }
    // Buffer is empty — check upstream lifecycle state.
    match state {
      // Open: upstream hasn't finished; use WouldBlock to tell the interpreter
      // "skip me this tick" without completing the source.
      | BoundaryState::Open => Err(StreamError::WouldBlock),
      // Completed: upstream finished and buffer drained; signal source exhaustion.
      | BoundaryState::Completed => Ok(None),
      // Failed: upstream failed and buffer drained; propagate the error.
      | BoundaryState::Failed(err) => Err(err),
    }
  }

  fn on_cancel(&mut self) -> Result<(), StreamError> {
    // downstream cancel を boundary 越しに伝播し、upstream 側の WouldBlock 張り付きを防ぐ。
    self.boundary.complete();
    Ok(())
  }
}
