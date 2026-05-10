use alloc::collections::VecDeque;

use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};

use crate::{QueueOfferResult, StreamError};

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
/// use fraktor_stream_core_kernel_rs::r#impl::queue::SourceQueue;
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
  pub fn offer(&mut self, value: T) -> QueueOfferResult {
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
  pub fn complete(&mut self) {
    let mut guard = self.inner.lock();
    guard.closed = true;
  }

  pub(crate) fn complete_if_open(&self) -> bool {
    let mut guard = self.inner.lock();
    if guard.closed {
      return false;
    }
    guard.closed = true;
    true
  }

  pub(crate) fn close_for_cancel(&self) {
    let mut guard = self.inner.lock();
    if guard.failure.is_some() {
      return;
    }
    guard.closed = true;
    guard.values.clear();
  }

  /// Fails the queue and rejects subsequent offers.
  pub fn fail(&mut self, error: StreamError) {
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

  /// Polls the next value, returning `Ok(None)` only when the queue is drained
  /// within a single lock acquisition. Avoids TOCTOU races.
  pub(crate) fn poll_or_drain(&self) -> Result<Option<T>, StreamError> {
    let mut guard = self.inner.lock();
    if let Some(error) = &guard.failure {
      return Err(error.clone());
    }
    match guard.values.pop_front() {
      | Some(value) => Ok(Some(value)),
      | None if guard.closed => Ok(None),
      | None => Err(StreamError::WouldBlock),
    }
  }

  /// Returns `true` when the queue is closed and all queued elements were consumed.
  #[must_use]
  pub fn is_drained(&self) -> bool {
    let guard = self.inner.lock();
    guard.closed && guard.values.is_empty()
  }
}

impl<T> Default for SourceQueue<T> {
  fn default() -> Self {
    Self::new()
  }
}
