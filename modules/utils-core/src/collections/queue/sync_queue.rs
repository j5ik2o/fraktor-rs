use core::marker::PhantomData;

use crate::collections::{
  PriorityMessage,
  queue::{
    QueueError,
    backend::{OfferOutcome, SyncQueueBackend, sync_priority_backend::SyncPriorityBackend},
    capabilities::SupportsPeek,
    type_keys::{FifoKey, MpscKey, PriorityKey, SpscKey, TypeKey},
  },
};

/// Queue API parameterised by element type, type key, backend, and shared guard.
#[derive(Clone)]
pub struct SyncQueue<T, K, B>
where
  K: TypeKey,
  B: SyncQueueBackend<T>, {
  backend: B,
  _pd:     PhantomData<(T, K, B)>,
}

impl<T, K, B> SyncQueue<T, K, B>
where
  K: TypeKey,
  B: SyncQueueBackend<T>,
{
  /// Creates a new queue from the provided shared backend.
  #[must_use]
  pub const fn new(backend: B) -> Self {
    Self { backend, _pd: PhantomData }
  }

  /// Enqueues an item according to the backend's overflow policy.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend rejects the element because the queue is closed,
  /// full, or disconnected.
  pub fn offer(&mut self, item: T) -> Result<OfferOutcome, QueueError<T>> {
    self.backend.offer(item)
  }

  /// Dequeues the next available item.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend cannot supply an element due to closure,
  /// disconnection, or backend-specific failures.
  pub fn poll(&mut self) -> Result<T, QueueError<T>> {
    self.backend.poll()
  }

  /// Requests the backend to transition into the closed state.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend refuses to close.
  pub fn close(&mut self) -> Result<(), QueueError<T>> {
    self.backend.close();
    Ok(())
  }

  /// Returns the current number of stored elements.
  #[must_use]
  pub fn len(&self) -> usize {
    self.backend.len()
  }

  /// Returns the storage capacity.
  #[must_use]
  pub fn capacity(&self) -> usize {
    self.backend.capacity()
  }

  /// Indicates whether the queue is empty.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }

  /// Indicates whether the queue is full.
  #[must_use]
  pub fn is_full(&self) -> bool {
    self.len() == self.capacity()
  }

  /// Provides access to the underlying shared backend.
  #[must_use]
  pub const fn backend(&self) -> &B {
    &self.backend
  }
}

impl<T, B> SyncQueue<T, PriorityKey, B>
where
  T: Clone + PriorityMessage,
  B: SyncPriorityBackend<T>,
  PriorityKey: SupportsPeek,
{
  /// Retrieves the smallest element without removing it.
  ///
  /// # Errors
  ///
  /// Returns a `QueueError` when the backend cannot access the next element due to closure,
  /// disconnection, or backend-specific failures.
  pub fn peek_min(&self) -> Result<Option<T>, QueueError<T>> {
    Ok(self.backend.peek_min().cloned())
  }
}

/// Type alias for an MPSC queue.
pub type SyncMpscQueue<T, B> = SyncQueue<T, MpscKey, B>;
/// Type alias for an SPSC queue.
pub type SyncSpscQueue<T, B> = SyncQueue<T, SpscKey, B>;
/// Type alias for a FIFO queue.
pub type SyncFifoQueue<T, B> = SyncQueue<T, FifoKey, B>;
/// Type alias for a priority queue.
pub type SyncPriorityQueue<T, B> = SyncQueue<T, PriorityKey, B>;
