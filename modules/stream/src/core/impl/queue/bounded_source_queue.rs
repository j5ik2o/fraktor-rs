use alloc::collections::VecDeque;

use fraktor_utils_rs::core::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

use crate::core::{OverflowStrategy, QueueOfferResult, r#impl::StreamError};

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
  pub fn offer(&mut self, value: T) -> QueueOfferResult {
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
  pub fn complete(&mut self) {
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
  pub fn fail(&mut self, error: StreamError) {
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

  /// Polls the next queued element without checking drained status.
  ///
  /// Prefer [`poll_or_drain`](Self::poll_or_drain) when the caller needs
  /// atomic poll + drained detection to avoid TOCTOU races.
  ///
  /// # Errors
  ///
  /// Returns the stored [`StreamError`] if the queue has been failed.
  pub fn poll(&self) -> Result<Option<T>, StreamError> {
    let mut guard = self.inner.lock();
    if let Some(error) = &guard.failure {
      return Err(error.clone());
    }
    Ok(guard.values.pop_front())
  }

  /// Polls the next value and checks drained status atomically.
  ///
  /// Returns:
  /// - `Ok(Some(value))` — a value was available
  /// - `Ok(None)` — the queue is drained (closed + empty); the stream is complete
  /// - `Err(WouldBlock)` — the queue is empty but not yet closed; retry later
  /// - `Err(error)` — the producer failed the queue
  ///
  /// Unlike calling `poll()` followed by `is_drained()`, this method performs
  /// both checks under a single lock acquisition, avoiding TOCTOU races where
  /// a concurrent producer thread sets `closed = true` between the two calls.
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
