use alloc::collections::VecDeque;

use fraktor_utils_rs::core::sync::{ArcShared, SpinSyncMutex};

#[cfg(test)]
mod tests;

struct SinkQueueInner<T> {
  queue:     VecDeque<T>,
  cancelled: bool,
}

/// Shared pull handle for queue-based sink materialization.
///
/// Elements pushed into the sink can be pulled from this handle.
/// Supports cancellation via [`cancel`](Self::cancel).
pub struct SinkQueue<T> {
  inner: ArcShared<SpinSyncMutex<SinkQueueInner<T>>>,
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
    Self { inner: ArcShared::new(SpinSyncMutex::new(SinkQueueInner { queue: VecDeque::new(), cancelled: false })) }
  }

  /// Pulls the next element from the queue.
  ///
  /// Returns `None` when the queue is empty or has been cancelled.
  #[must_use]
  pub fn pull(&self) -> Option<T> {
    let mut guard = self.inner.lock();
    if guard.cancelled {
      return None;
    }
    guard.queue.pop_front()
  }

  /// Returns the number of elements currently in the queue.
  #[must_use]
  pub fn len(&self) -> usize {
    let guard = self.inner.lock();
    guard.queue.len()
  }

  /// Returns `true` when the queue contains no elements.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    let guard = self.inner.lock();
    guard.queue.is_empty()
  }

  /// Cancels the stream by clearing the queue and rejecting further pulls.
  ///
  /// This method returns immediately without waiting for finalization.
  /// After cancellation, [`pull`](Self::pull) always returns `None` and
  /// [`push`](Self::push) silently discards elements.
  pub fn cancel(&mut self) {
    let mut guard = self.inner.lock();
    guard.cancelled = true;
    guard.queue.clear();
  }

  /// Returns `true` when the queue has been cancelled.
  #[must_use]
  pub fn is_cancelled(&self) -> bool {
    let guard = self.inner.lock();
    guard.cancelled
  }

  pub(crate) fn push(&mut self, value: T) {
    let mut guard = self.inner.lock();
    if guard.cancelled {
      return;
    }
    guard.queue.push_back(value);
  }
}

impl<T> Default for SinkQueue<T> {
  fn default() -> Self {
    Self::new()
  }
}
