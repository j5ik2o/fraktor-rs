#[cfg(test)]
mod tests;

use alloc::collections::BinaryHeap;
use core::cmp;

use crate::collections::queue::{
  OfferOutcome, OverflowPolicy, QueueError, SyncQueueBackend, backend::SyncQueueBackendInternal,
};

/// Queue backend backed by [`BinaryHeap`].
///
/// Elements are automatically ordered by their [`Ord`] implementation.
/// This backend provides priority queue semantics where `poll` returns the maximum element.
pub struct BinaryHeapBackend<T: Ord> {
  heap:   BinaryHeap<T>,
  limit:  usize,
  policy: OverflowPolicy,
  closed: bool,
}

impl<T: Ord> BinaryHeapBackend<T> {
  /// Creates a backend with the specified capacity limit and overflow policy.
  #[must_use]
  pub fn with_capacity(capacity: usize, policy: OverflowPolicy) -> Self {
    Self { heap: BinaryHeap::with_capacity(capacity), limit: capacity, policy, closed: false }
  }

  fn ensure_capacity(&mut self, required: usize) -> Option<usize> {
    if required <= self.limit {
      return None;
    }

    let current = self.limit;
    let next = cmp::max(required, cmp::max(1, current.saturating_mul(2)));
    self.heap.reserve(next - self.heap.len());
    self.limit = next;
    Some(next)
  }

  fn handle_full_queue(&mut self, item: T) -> Result<OfferOutcome, QueueError<T>> {
    match self.policy {
      | OverflowPolicy::DropNewest => {
        drop(item);
        Ok(OfferOutcome::DroppedNewest { count: 1 })
      },
      | OverflowPolicy::DropOldest => {
        // For BinaryHeap, "oldest" is interpreted as the maximum element
        let _ = self.heap.pop();
        self.heap.push(item);
        Ok(OfferOutcome::DroppedOldest { count: 1 })
      },
      | OverflowPolicy::Block => Err(QueueError::Full(item)),
      | OverflowPolicy::Grow => {
        let grown_to = self.handle_grow_policy(item);
        Ok(OfferOutcome::GrewTo { capacity: grown_to })
      },
    }
  }

  fn handle_grow_policy(&mut self, item: T) -> usize {
    let required = self.heap.len().saturating_add(1);
    match self.ensure_capacity(required) {
      | Some(capacity) => {
        self.heap.push(item);
        capacity
      },
      | None => {
        self.heap.push(item);
        self.limit
      },
    }
  }
}

impl<T: Ord> SyncQueueBackend<T> for BinaryHeapBackend<T> {}

impl<T: Ord> SyncQueueBackendInternal<T> for BinaryHeapBackend<T> {
  fn offer(&mut self, item: T) -> Result<OfferOutcome, QueueError<T>> {
    if self.closed {
      return Err(QueueError::Closed(item));
    }

    if self.heap.len() >= self.limit {
      return self.handle_full_queue(item);
    }

    self.heap.push(item);
    Ok(OfferOutcome::Enqueued)
  }

  fn poll(&mut self) -> Result<T, QueueError<T>> {
    match self.heap.pop() {
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
    self.heap.len()
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
