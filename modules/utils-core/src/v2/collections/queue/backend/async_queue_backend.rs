use alloc::boxed::Box;

use async_trait::async_trait;

use super::OfferOutcome;
use crate::{collections::queue::QueueError, v2::collections::wait::WaitHandle};

/// Async-compatible backend trait for queue operations.
#[async_trait(?Send)]
pub trait AsyncQueueBackend<T> {
  /// Adds an element to the queue according to the configured overflow policy.
  async fn offer(&mut self, item: T) -> Result<OfferOutcome, QueueError<T>>;

  /// Removes and returns the next element from the queue.
  async fn poll(&mut self) -> Result<T, QueueError<T>>;

  /// Transitions the backend into the closed state.
  async fn close(&mut self) -> Result<(), QueueError<T>>;

  /// Returns the number of elements currently stored.
  fn len(&self) -> usize;

  /// Returns the maximum number of elements that can be stored without growing.
  fn capacity(&self) -> usize;

  /// Indicates whether the queue is empty.
  fn is_empty(&self) -> bool {
    self.len() == 0
  }

  /// Indicates whether the queue is full.
  fn is_full(&self) -> bool {
    self.len() == self.capacity()
  }

  /// Optionally registers a producer waiter when the queue is full.
  fn prepare_producer_wait(&mut self) -> Option<WaitHandle<QueueError<T>>> {
    let _ = self;
    None
  }

  /// Optionally registers a consumer waiter when the queue is empty.
  fn prepare_consumer_wait(&mut self) -> Option<WaitHandle<QueueError<T>>> {
    let _ = self;
    None
  }

  /// Indicates whether the backend has transitioned into the closed state.
  fn is_closed(&self) -> bool {
    false
  }
}
