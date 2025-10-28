use core::cmp;

use crate::{
  collections::queue::QueueError,
  v2::collections::queue::{OfferOutcome, OverflowPolicy, QueueStorage, SyncQueueBackend, VecRingStorage},
};

/// Queue backend backed by a ring buffer storage.
pub struct VecRingBackend<T> {
  storage: VecRingStorage<T>,
  policy:  OverflowPolicy,
  closed:  bool,
}

impl<T> VecRingBackend<T> {
  /// Creates a backend from the provided storage and overflow policy.
  #[must_use]
  pub const fn new_with_storage(storage: VecRingStorage<T>, policy: OverflowPolicy) -> Self {
    Self { storage, policy, closed: false }
  }

  fn ensure_capacity(&mut self, required: usize) -> Result<Option<usize>, ()> {
    if required <= self.storage.capacity() {
      return Ok(None);
    }

    let current = self.storage.capacity();
    let next = cmp::max(required, cmp::max(1, current.saturating_mul(2)));
    self.storage.try_grow(next).map_err(|_| ())?;
    Ok(Some(next))
  }

  fn handle_full_queue(&mut self, item: T) -> Result<OfferOutcome, QueueError<T>> {
    match self.policy {
      | OverflowPolicy::DropNewest => {
        drop(item);
        Ok(OfferOutcome::DroppedNewest { count: 1 })
      },
      | OverflowPolicy::DropOldest => {
        let _ = self.storage.pop_front();
        self.storage.push_back(item);
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
    let required = self.storage.len().saturating_add(1);
    match self.ensure_capacity(required) {
      | Ok(Some(capacity)) => {
        self.storage.push_back(item);
        Ok(capacity)
      },
      | Ok(None) => {
        self.storage.push_back(item);
        Ok(self.storage.capacity())
      },
      | Err(()) => Err(QueueError::AllocError(item)),
    }
  }
}

impl<T> SyncQueueBackend<T> for VecRingBackend<T> {
  type Storage = VecRingStorage<T>;

  fn new(storage: Self::Storage, policy: OverflowPolicy) -> Self {
    VecRingBackend::new_with_storage(storage, policy)
  }

  fn offer(&mut self, item: T) -> Result<OfferOutcome, QueueError<T>> {
    if self.closed {
      return Err(QueueError::Closed(item));
    }

    if self.storage.is_full() {
      return self.handle_full_queue(item);
    }

    self.storage.push_back(item);
    Ok(OfferOutcome::Enqueued)
  }

  fn poll(&mut self) -> Result<T, QueueError<T>> {
    match self.storage.pop_front() {
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
    self.storage.len()
  }

  fn capacity(&self) -> usize {
    self.storage.capacity()
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
