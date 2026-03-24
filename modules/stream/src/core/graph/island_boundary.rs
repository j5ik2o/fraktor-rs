//! Bounded buffer for inter-island element transfer.
//!
//! `IslandBoundary` provides a shared, capacity-limited FIFO buffer used to
//! bridge two independently-driven islands within a stream graph. Completion
//! and error states are propagated through the buffer so the downstream island
//! can detect end-of-stream or failure after draining remaining elements.

use alloc::collections::VecDeque;
use core::ops::DerefMut;

use fraktor_utils_rs::core::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

use crate::core::{DynValue, StreamError};

#[cfg(test)]
mod tests;

/// Default capacity for inter-island boundary buffers.
pub(crate) const DEFAULT_BOUNDARY_CAPACITY: usize = 16;

/// Lifecycle state of an `IslandBoundary`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BoundaryState {
  /// The boundary is open for push/pull.
  Open,
  /// The upstream has completed normally. Remaining buffered elements
  /// should be drained before the downstream observes completion.
  Completed,
  /// The upstream has failed. Remaining buffered elements should be
  /// drained before the downstream observes the error.
  Failed(StreamError),
}

/// Bounded FIFO buffer between two islands.
///
/// Elements are pushed by a `BoundarySinkLogic` (upstream island) and
/// pulled by a `BoundarySourceLogic` (downstream island). The buffer
/// enforces a maximum capacity for backpressure.
pub(crate) struct IslandBoundary {
  buffer:   VecDeque<DynValue>,
  capacity: usize,
  state:    BoundaryState,
}

impl IslandBoundary {
  /// Creates a new boundary with the given capacity.
  #[must_use]
  pub(crate) fn new(capacity: usize) -> Self {
    Self { buffer: VecDeque::with_capacity(capacity), capacity, state: BoundaryState::Open }
  }

  /// Returns the number of buffered elements.
  #[must_use]
  #[allow(dead_code)]
  pub(crate) fn len(&self) -> usize {
    self.buffer.len()
  }

  /// Returns `true` if no elements are buffered.
  #[must_use]
  #[allow(dead_code)]
  pub(crate) fn is_empty(&self) -> bool {
    self.buffer.is_empty()
  }

  /// Returns the current lifecycle state.
  #[must_use]
  pub(crate) const fn state(&self) -> &BoundaryState {
    &self.state
  }

  /// Attempts to push a value into the buffer.
  ///
  /// Returns `Ok(())` on success. Returns `Err(value)` when the buffer
  /// is full or the boundary is no longer open, giving the value back to
  /// the caller.
  pub(crate) fn try_push(&mut self, value: DynValue) -> Result<(), DynValue> {
    if !matches!(self.state, BoundaryState::Open) {
      return Err(value);
    }
    if self.buffer.len() >= self.capacity {
      return Err(value);
    }
    self.buffer.push_back(value);
    Ok(())
  }

  /// Attempts to pull a value from the buffer.
  ///
  /// Returns `Some(value)` if an element is available, `None` otherwise.
  /// The caller should check `state()` when `None` is returned to
  /// distinguish "empty but open" from "completed" or "failed".
  pub(crate) fn try_pull(&mut self) -> Option<DynValue> {
    self.buffer.pop_front()
  }

  /// Marks the boundary as completed.
  ///
  /// Remaining buffered elements can still be pulled. Idempotent: calling
  /// `complete()` on an already-completed boundary is a no-op.
  pub(crate) fn complete(&mut self) {
    if matches!(self.state, BoundaryState::Open) {
      self.state = BoundaryState::Completed;
    }
  }

  /// Marks the boundary as failed.
  ///
  /// Remaining buffered elements can still be pulled before the error
  /// surfaces.
  pub(crate) fn fail(&mut self, error: StreamError) {
    if matches!(self.state, BoundaryState::Open) {
      self.state = BoundaryState::Failed(error);
    }
  }
}

/// Shared, clone-able handle to an `IslandBoundary`.
///
/// Uses `ArcShared<SpinSyncMutex<IslandBoundary>>` for lock-based access
/// because `try_push` returns ownership of the rejected value, which
/// cannot be expressed through a `SharedAccess`-style closure API.
#[derive(Clone)]
pub(crate) struct IslandBoundaryShared {
  inner: ArcShared<SpinSyncMutex<IslandBoundary>>,
}

impl IslandBoundaryShared {
  /// Creates a new shared boundary with the given capacity.
  #[must_use]
  pub(crate) fn new(capacity: usize) -> Self {
    Self { inner: ArcShared::new(SpinSyncMutex::new(IslandBoundary::new(capacity))) }
  }

  /// Acquires the spinlock and returns a guard.
  ///
  /// The returned guard dereferences to `&mut IslandBoundary`.
  pub(crate) fn lock(&self) -> impl DerefMut<Target = IslandBoundary> + '_ {
    self.inner.lock()
  }
}
