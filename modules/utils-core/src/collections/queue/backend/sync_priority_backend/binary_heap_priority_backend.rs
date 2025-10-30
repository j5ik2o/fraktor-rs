use alloc::collections::BinaryHeap;
use core::cmp::Ordering;

use super::{SyncPriorityBackend, priority_backend_config::PriorityBackendConfig, priority_entry::PriorityEntry};
use crate::collections::{
  PriorityMessage,
  queue::{OfferOutcome, OverflowPolicy, QueueError, SyncQueueBackend},
};

/// Priority-aware backend backed by a binary heap.
pub struct BinaryHeapPriorityBackend<T: PriorityMessage> {
  heap:          BinaryHeap<PriorityEntry<T>>,
  config:        PriorityBackendConfig,
  limit:         usize,
  policy:        OverflowPolicy,
  closed:        bool,
  next_sequence: u64,
}

impl<T: PriorityMessage> BinaryHeapPriorityBackend<T> {
  /// Creates a backend configured with explicit priority bounds.
  #[must_use]
  pub fn new_with_config(config: PriorityBackendConfig, policy: OverflowPolicy) -> Self {
    let capacity = config.capacity();
    Self { heap: BinaryHeap::with_capacity(capacity), limit: capacity, policy, closed: false, next_sequence: 0, config }
  }

  /// Creates a backend using the default priority layout.
  #[must_use]
  pub fn new_with_capacity(capacity: usize, policy: OverflowPolicy) -> Self {
    Self::new_with_config(PriorityBackendConfig::with_default_layout(capacity), policy)
  }

  fn determine_priority(&self, item: &T) -> i8 {
    let raw = item.get_priority().unwrap_or(self.config.default_priority());
    self.config.clamp_priority(raw)
  }

  #[allow(clippy::missing_const_for_fn)]
  fn allocate_sequence(&mut self) -> u64 {
    let seq = self.next_sequence;
    self.next_sequence = self.next_sequence.wrapping_add(1);
    seq
  }

  fn push_entry(&mut self, entry: PriorityEntry<T>) {
    self.heap.push(entry);
  }
}

impl<T: PriorityMessage> SyncQueueBackend<T> for BinaryHeapPriorityBackend<T> {
  type Storage = PriorityBackendConfig;

  fn new(storage: Self::Storage, policy: OverflowPolicy) -> Self {
    BinaryHeapPriorityBackend::new_with_config(storage, policy)
  }

  fn offer(&mut self, item: T) -> Result<OfferOutcome, QueueError<T>> {
    if self.closed {
      return Err(QueueError::Closed(item));
    }

    let priority = self.determine_priority(&item);

    if self.heap.len() < self.limit {
      let sequence = self.allocate_sequence();
      self.push_entry(PriorityEntry::new(priority, sequence, item));
      return Ok(OfferOutcome::Enqueued);
    }

    match self.policy {
      | OverflowPolicy::DropNewest => {
        drop(item);
        Ok(OfferOutcome::DroppedNewest { count: 1 })
      },
      | OverflowPolicy::DropOldest => {
        let sequence = self.allocate_sequence();
        let entry = PriorityEntry::new(priority, sequence, item);
        let _ = self.heap.pop();
        self.push_entry(entry);
        Ok(OfferOutcome::DroppedOldest { count: 1 })
      },
      | OverflowPolicy::Block => Err(QueueError::Full(item)),
      | OverflowPolicy::Grow => {
        let sequence = self.allocate_sequence();
        let entry = PriorityEntry::new(priority, sequence, item);
        self.limit = self.limit.saturating_add(1);
        self.push_entry(entry);
        Ok(OfferOutcome::GrewTo { capacity: self.limit })
      },
    }
  }

  fn poll(&mut self) -> Result<T, QueueError<T>> {
    match self.heap.pop() {
      | Some(entry) => Ok(entry.into_item()),
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

impl<T: PriorityMessage> SyncPriorityBackend<T> for BinaryHeapPriorityBackend<T> {
  fn peek_min(&self) -> Option<&T> {
    self
      .heap
      .iter()
      .min_by(|a, b| match a.priority().cmp(&b.priority()) {
        | Ordering::Equal => a.sequence().cmp(&b.sequence()),
        | ord => ord,
      })
      .map(|entry| entry.item())
  }
}
