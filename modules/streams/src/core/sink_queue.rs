use alloc::collections::VecDeque;

use fraktor_utils_rs::core::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

#[cfg(test)]
mod tests;

/// Shared pull handle for queue-based sink materialization.
///
/// Elements pushed into the sink can be pulled from this handle.
pub struct SinkQueue<T> {
  inner: ArcShared<SpinSyncMutex<VecDeque<T>>>,
}

impl<T> Clone for SinkQueue<T> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<T> SinkQueue<T> {
  /// Creates an empty sink queue.
  #[must_use]
  pub fn new() -> Self {
    Self { inner: ArcShared::new(SpinSyncMutex::new(VecDeque::new())) }
  }

  /// Pulls the next element from the queue.
  #[must_use]
  pub fn pull(&self) -> Option<T> {
    let mut guard = self.inner.lock();
    guard.pop_front()
  }

  /// Returns the number of elements currently in the queue.
  #[must_use]
  pub fn len(&self) -> usize {
    let guard = self.inner.lock();
    guard.len()
  }

  /// Returns `true` when the queue contains no elements.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    let guard = self.inner.lock();
    guard.is_empty()
  }

  pub(crate) fn push(&self, value: T) {
    let mut guard = self.inner.lock();
    guard.push_back(value);
  }
}

impl<T> Default for SinkQueue<T> {
  fn default() -> Self {
    Self::new()
  }
}
