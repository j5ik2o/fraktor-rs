use core::marker::PhantomData;

use crate::collections::queue::{QueueError, backend::SyncQueueBackend, offer_outcome::OfferOutcome};

/// Generic queue API parameterised by element type and backend.
///
/// The queue itself is variant-agnostic; ordering and capability semantics
/// (FIFO / priority / etc.) are entirely determined by the supplied backend
/// implementation.
#[derive(Clone)]
pub struct SyncQueue<T, B>
where
  B: SyncQueueBackend<T>, {
  backend: B,
  // The where clause `B: SyncQueueBackend<T>` does NOT count as a use of `T`
  // per rustc's E0392 check (only struct fields do). We have no field whose
  // type names `T` directly, so `PhantomData<T>` is required to keep `T` in
  // the struct definition.
  _pd:     PhantomData<T>,
}

impl<T, B> SyncQueue<T, B>
where
  B: SyncQueueBackend<T>,
{
  /// Creates a new queue from the provided backend.
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

  /// Provides access to the underlying backend.
  #[must_use]
  pub const fn backend(&self) -> &B {
    &self.backend
  }
}
