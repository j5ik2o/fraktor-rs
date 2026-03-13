use alloc::collections::VecDeque;

use fraktor_utils_rs::core::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

use super::{OverflowStrategy, QueueOfferResult, StreamError};

#[cfg(test)]
mod tests;

struct BoundedSourceQueueState<T> {
  values:  VecDeque<T>,
  closed:  bool,
  failure: Option<StreamError>,
}

/// Bounded queue materialized by `Source::queue`.
pub struct BoundedSourceQueue<T> {
  inner:             ArcShared<SpinSyncMutex<BoundedSourceQueueState<T>>>,
  capacity:          usize,
  overflow_strategy: OverflowStrategy,
}

impl<T> Clone for BoundedSourceQueue<T> {
  fn clone(&self) -> Self {
    Self {
      inner:             self.inner.clone(),
      capacity:          self.capacity,
      overflow_strategy: self.overflow_strategy,
    }
  }
}

impl<T> BoundedSourceQueue<T> {
  /// Creates an empty bounded queue.
  ///
  /// # Panics
  ///
  /// Panics when `capacity` is zero.
  #[must_use]
  pub fn new(capacity: usize, overflow_strategy: OverflowStrategy) -> Self {
    assert!(capacity > 0, "capacity must be greater than zero");
    let state = BoundedSourceQueueState { values: VecDeque::new(), closed: false, failure: None };
    Self { inner: ArcShared::new(SpinSyncMutex::new(state)), capacity, overflow_strategy }
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
    if guard.values.len() < self.capacity {
      guard.values.push_back(value);
      return QueueOfferResult::Enqueued;
    }

    match self.overflow_strategy {
      | OverflowStrategy::Backpressure => QueueOfferResult::Failure(StreamError::WouldBlock),
      | OverflowStrategy::DropHead => {
        let _ = guard.values.pop_front();
        guard.values.push_back(value);
        QueueOfferResult::Enqueued
      },
      | OverflowStrategy::DropTail => {
        let _ = guard.values.pop_back();
        guard.values.push_back(value);
        QueueOfferResult::Enqueued
      },
      | OverflowStrategy::DropBuffer => {
        guard.values.clear();
        guard.values.push_back(value);
        QueueOfferResult::Enqueued
      },
      | OverflowStrategy::Fail => {
        let error = StreamError::BufferOverflow;
        guard.failure = Some(error.clone());
        guard.closed = true;
        QueueOfferResult::Failure(error)
      },
    }
  }

  /// Completes the queue and rejects subsequent offers.
  ///
  /// # Panics
  ///
  /// Panics when the queue has already been completed or failed.
  pub fn complete(&self) {
    assert!(self.complete_if_open(), "bounded source queue already terminated: complete");
  }

  pub(crate) fn complete_if_open(&self) -> bool {
    let mut guard = self.inner.lock();
    if guard.closed || guard.failure.is_some() {
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
  ///
  /// # Panics
  ///
  /// Panics when the queue has already been completed or failed.
  pub fn fail(&self, error: StreamError) {
    assert!(self.fail_if_open(error), "bounded source queue already terminated: fail");
  }

  pub(crate) fn fail_if_open(&self, error: StreamError) -> bool {
    let mut guard = self.inner.lock();
    if guard.closed || guard.failure.is_some() {
      return false;
    }
    guard.failure = Some(error);
    guard.closed = true;
    true
  }

  /// Returns the configured capacity.
  #[must_use]
  pub const fn capacity(&self) -> usize {
    self.capacity
  }

  /// Returns the configured overflow strategy.
  #[must_use]
  pub const fn overflow_strategy(&self) -> OverflowStrategy {
    self.overflow_strategy
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

  /// Returns `true` when the queue is closed.
  #[must_use]
  pub fn is_closed(&self) -> bool {
    let guard = self.inner.lock();
    guard.closed
  }

  pub(crate) fn poll(&self) -> Result<Option<T>, StreamError> {
    let mut guard = self.inner.lock();
    if let Some(error) = &guard.failure {
      return Err(error.clone());
    }
    Ok(guard.values.pop_front())
  }

  pub(crate) fn is_drained(&self) -> bool {
    let guard = self.inner.lock();
    guard.closed && guard.values.is_empty()
  }
}
