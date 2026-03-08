use alloc::collections::VecDeque;

use fraktor_utils_rs::core::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

use super::{QueueOfferResult, StreamError};

#[cfg(test)]
mod tests;

struct SourceQueueState<T> {
  values:  VecDeque<T>,
  closed:  bool,
  failure: Option<StreamError>,
}

/// Unbounded queue materialized by source queue APIs.
///
/// Internal dequeue helpers stay crate-private.
///
/// ```compile_fail
/// use fraktor_streams_rs::core::SourceQueue;
///
/// let queue = SourceQueue::<u32>::new();
/// let _ = queue.poll();
/// let _ = queue.is_drained();
/// ```
pub struct SourceQueue<T> {
  inner: ArcShared<SpinSyncMutex<SourceQueueState<T>>>,
}

impl<T> Clone for SourceQueue<T> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<T> SourceQueue<T> {
  /// Creates an empty queue.
  #[must_use]
  pub fn new() -> Self {
    let state = SourceQueueState { values: VecDeque::new(), closed: false, failure: None };
    Self { inner: ArcShared::new(SpinSyncMutex::new(state)) }
  }

  /// Offers a value into the queue.
  #[must_use]
  pub fn offer(&self, value: T) -> QueueOfferResult {
    let mut guard = self.inner.lock();
    if let Some(error) = &guard.failure {
      return QueueOfferResult::Failure(error.clone());
    }
    if guard.closed {
      return QueueOfferResult::QueueClosed;
    }
    guard.values.push_back(value);
    QueueOfferResult::Enqueued
  }

  /// Completes the queue and rejects subsequent offers.
  pub fn complete(&self) {
    let mut guard = self.inner.lock();
    guard.closed = true;
  }

  /// Fails the queue and rejects subsequent offers.
  pub fn fail(&self, error: StreamError) {
    let mut guard = self.inner.lock();
    guard.failure = Some(error);
    guard.closed = true;
  }

  /// Returns `true` when the queue is closed.
  #[must_use]
  pub fn is_closed(&self) -> bool {
    let guard = self.inner.lock();
    guard.closed
  }

  /// Returns the number of queued elements.
  #[must_use]
  pub fn len(&self) -> usize {
    let guard = self.inner.lock();
    guard.values.len()
  }

  /// Returns `true` when the queue contains no elements.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }

  /// Polls the next queued element.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] if the queue has been failed.
  pub(crate) fn poll(&self) -> Result<Option<T>, StreamError> {
    let mut guard = self.inner.lock();
    if let Some(error) = &guard.failure {
      return Err(error.clone());
    }
    Ok(guard.values.pop_front())
  }

  /// Returns `true` when the queue is closed and all queued elements were consumed.
  #[must_use]
  pub(crate) fn is_drained(&self) -> bool {
    let guard = self.inner.lock();
    guard.closed && guard.values.is_empty()
  }
}

impl<T> Default for SourceQueue<T> {
  fn default() -> Self {
    Self::new()
  }
}
