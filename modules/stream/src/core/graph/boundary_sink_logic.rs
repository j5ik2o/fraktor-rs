//! Sink logic for the upstream side of an island boundary.
//!
//! `BoundarySinkLogic` implements `SinkLogic` and forwards incoming elements
//! into a shared `IslandBoundaryShared`. When the boundary buffer is full,
//! the element is held as pending and retried on subsequent ticks.

use super::island_boundary::IslandBoundaryShared;
use crate::core::{DemandTracker, DynValue, SinkDecision, SinkLogic, StreamError};

#[cfg(test)]
mod tests;

/// Deferred terminal signal recorded while a pending element is waiting to be flushed.
enum PendingTerminal {
  Complete,
  Failed(StreamError),
}

/// Sink stage logic that pushes elements into an inter-island boundary buffer.
pub(crate) struct BoundarySinkLogic {
  boundary:         IslandBoundaryShared,
  pending:          Option<DynValue>,
  pending_terminal: Option<PendingTerminal>,
}

impl BoundarySinkLogic {
  /// Creates a new boundary sink logic connected to the given shared boundary.
  #[must_use]
  pub(crate) fn new(boundary: IslandBoundaryShared) -> Self {
    Self { boundary, pending: None, pending_terminal: None }
  }
}

impl SinkLogic for BoundarySinkLogic {
  fn on_start(&mut self, demand: &mut DemandTracker) -> Result<(), StreamError> {
    demand.request(1)
  }

  fn on_push(&mut self, input: DynValue, demand: &mut DemandTracker) -> Result<SinkDecision, StreamError> {
    let mut guard = self.boundary.lock();
    match guard.try_push(input) {
      | Ok(()) => {
        drop(guard);
        demand.request(1)?;
        Ok(SinkDecision::Continue)
      },
      | Err(rejected) => {
        self.pending = Some(rejected);
        Ok(SinkDecision::Continue)
      },
    }
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    if self.pending.is_some() {
      // pending 要素がまだ flush されていないため、終端を遅延させる。
      // on_tick で pending push 成功後に boundary を閉じる。
      self.pending_terminal = Some(PendingTerminal::Complete);
    } else {
      let mut guard = self.boundary.lock();
      guard.complete();
    }
    Ok(())
  }

  fn on_error(&mut self, error: StreamError) {
    if self.pending.is_some() {
      // pending 要素がまだ flush されていないため、終端を遅延させる。
      self.pending_terminal = Some(PendingTerminal::Failed(error));
    } else {
      let mut guard = self.boundary.lock();
      guard.fail(error);
    }
  }

  fn on_tick(&mut self, demand: &mut DemandTracker) -> Result<bool, StreamError> {
    let Some(value) = self.pending.take() else {
      return Ok(false);
    };
    let mut guard = self.boundary.lock();
    match guard.try_push(value) {
      | Ok(()) => {
        // pending flush 成功 — 遅延された終端信号があれば適用する。
        if let Some(terminal) = self.pending_terminal.take() {
          match terminal {
            | PendingTerminal::Complete => guard.complete(),
            | PendingTerminal::Failed(err) => guard.fail(err),
          }
        }
        drop(guard);
        demand.request(1)?;
        Ok(true)
      },
      | Err(rejected) => {
        self.pending = Some(rejected);
        Ok(false)
      },
    }
  }

  fn has_pending_work(&self) -> bool {
    self.pending.is_some() || self.pending_terminal.is_some()
  }
}
