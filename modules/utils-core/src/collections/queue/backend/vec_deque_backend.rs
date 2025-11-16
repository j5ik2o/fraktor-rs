#[cfg(test)]
mod tests;

use alloc::collections::{TryReserveError, VecDeque};
use core::cmp;

use crate::collections::queue::{
  OfferOutcome, OverflowPolicy, QueueError, SyncQueueBackend, backend::SyncQueueBackendInternal,
};

/// Queue backend backed by [`VecDeque`].
///
/// This adapter is meant to be constructed and driven by `AsyncQueue`/`SyncQueue`
/// helpers. Prefer those high-level APIs and implement custom backends instead of
/// invoking this adapter directly from application logic.
pub struct VecDequeBackend<T> {
  buffer: VecDeque<T>,
  limit:  usize,
  policy: OverflowPolicy,
  closed: bool,
}

impl<T> VecDequeBackend<T> {
  /// Creates a backend with the specified capacity limit and overflow policy.
  #[must_use]
  pub fn with_capacity(capacity: usize, policy: OverflowPolicy) -> Self {
    Self { buffer: VecDeque::with_capacity(capacity), limit: capacity, policy, closed: false }
  }

  /// Returns the number of stored elements.
  #[must_use]
  fn len_internal(&self) -> usize {
    self.buffer.len()
  }

  /// Indicates whether the storage is full.
  #[must_use]
  fn is_full_internal(&self) -> bool {
    self.len_internal() == self.limit
  }

  /// Pushes an element to the back of the buffer.
  fn push_back(&mut self, value: T) {
    debug_assert!(!self.is_full_internal());
    self.buffer.push_back(value);
  }

  /// Pops an element from the front of the buffer.
  fn pop_front(&mut self) -> Option<T> {
    self.buffer.pop_front()
  }

  /// Attempts to grow the capacity limit to the provided value.
  fn try_grow(&mut self, new_capacity: usize) -> Result<(), TryReserveError> {
    if new_capacity <= self.limit {
      return Ok(());
    }
    let additional = new_capacity - self.limit;
    self.buffer.try_reserve(additional)?;
    self.limit = new_capacity;
    Ok(())
  }

  fn ensure_capacity(&mut self, required: usize) -> Result<Option<usize>, ()> {
    if required <= self.limit {
      return Ok(None);
    }

    let current = self.limit;
    let next = cmp::max(required, cmp::max(1, current.saturating_mul(2)));
    self.try_grow(next).map_err(|_| ())?;
    Ok(Some(next))
  }

  fn handle_full_queue(&mut self, item: T) -> Result<OfferOutcome, QueueError<T>> {
    match self.policy {
      | OverflowPolicy::DropNewest => {
        drop(item);
        Ok(OfferOutcome::DroppedNewest { count: 1 })
      },
      | OverflowPolicy::DropOldest => {
        let _ = self.pop_front();
        self.push_back(item);
        Ok(OfferOutcome::DroppedOldest { count: 1 })
      },
      | OverflowPolicy::Block => Err(QueueError::Full(item)),
      | OverflowPolicy::Grow => {
        let grown_to = self.handle_grow_policy(item)?;
        Ok(OfferOutcome::GrewTo { capacity: grown_to })
      },
    }
  }

  fn handle_grow_policy(&mut self, item: T) -> Result<usize, QueueError<T>> {
    let required = self.len_internal().saturating_add(1);
    match self.ensure_capacity(required) {
      | Ok(Some(capacity)) => {
        self.push_back(item);
        Ok(capacity)
      },
      | Ok(None) => {
        self.push_back(item);
        Ok(self.limit)
      },
      | Err(()) => Err(QueueError::AllocError(item)),
    }
  }
}

impl<T> SyncQueueBackend<T> for VecDequeBackend<T> {}

impl<T> SyncQueueBackendInternal<T> for VecDequeBackend<T> {
  fn offer(&mut self, item: T) -> Result<OfferOutcome, QueueError<T>> {
    if self.closed {
      return Err(QueueError::Closed(item));
    }

    if self.is_full_internal() {
      return self.handle_full_queue(item);
    }

    self.push_back(item);
    Ok(OfferOutcome::Enqueued)
  }

  fn poll(&mut self) -> Result<T, QueueError<T>> {
    match self.pop_front() {
      | Some(item) => Ok(item),
      | None => {
        if self.closed {
          Err(QueueError::Disconnected)
        } else {
          Err(QueueError::Empty)
        }
      },
    }
  }

  fn len(&self) -> usize {
    self.len_internal()
  }

  fn capacity(&self) -> usize {
    self.limit
  }

  fn overflow_policy(&self) -> OverflowPolicy {
    self.policy
  }

  fn is_closed(&self) -> bool {
    self.closed
  }

  fn close(&mut self) {
    self.closed = true;
  }
}
