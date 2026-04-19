//! Handles for interacting with queue producers/consumers.

use core::cmp;

use fraktor_utils_core_rs::core::{
  collections::queue::{OfferOutcome, OverflowPolicy, QueueError, SyncQueue, backend::VecDequeBackend},
  sync::{SharedAccess, SharedLock},
};

use super::mailbox_queue_state::{QueueState, queue_state_shared};
use crate::core::kernel::dispatch::mailbox::{
  capacity::MailboxCapacity, overflow_strategy::MailboxOverflowStrategy, policy::MailboxPolicy,
};

const DEFAULT_QUEUE_CAPACITY: usize = 16;

/// Result of [`QueueStateHandle::drop_oldest_and_offer`] that surfaces the
/// evicted element so callers (mailbox layer) can forward it to dead letters.
pub(crate) enum DropOldestOutcome<T> {
  /// The new element was offered without evicting an existing entry.
  Accepted,
  /// The new element was offered after evicting the oldest entry, which
  /// is returned so the caller can forward it to dead letters.
  Evicted(T),
}

/// Internal handles wrapping queue producers/consumers.
pub(crate) struct QueueStateHandle<T>
where
  T: Send + 'static, {
  pub(crate) state: SharedLock<QueueState<T>>,
}

impl<T> Clone for QueueStateHandle<T>
where
  T: Send + 'static,
{
  fn clone(&self) -> Self {
    Self { state: self.state.clone() }
  }
}

impl<T> QueueStateHandle<T>
where
  T: Send + 'static,
{
  pub(crate) fn new_user(policy: &MailboxPolicy) -> Self {
    let (capacity, overflow) = match policy.capacity() {
      | MailboxCapacity::Bounded { capacity } => (cmp::max(1, capacity.get()), map_overflow(policy.overflow())),
      | MailboxCapacity::Unbounded => (DEFAULT_QUEUE_CAPACITY, OverflowPolicy::Grow),
    };
    Self::new_with(capacity, overflow)
  }

  fn new_with(capacity: usize, overflow: OverflowPolicy) -> Self {
    let backend = VecDequeBackend::with_capacity(capacity, overflow);
    let sync_queue = SyncQueue::new(backend);
    let state = queue_state_shared(sync_queue);
    Self { state }
  }

  pub(crate) fn offer(&self, message: T) -> Result<OfferOutcome, QueueError<T>> {
    self.state.with_write(|state| state.offer(message))
  }

  pub(crate) fn offer_if_room(&self, message: T, capacity: usize) -> Result<OfferOutcome, QueueError<T>> {
    self.state.with_write(|state| {
      if state.len() >= capacity {
        return Err(QueueError::Full(message));
      }
      state.offer(message)
    })
  }

  pub(crate) fn drop_oldest_and_offer(
    &self,
    message: T,
    capacity: usize,
  ) -> Result<DropOldestOutcome<T>, QueueError<T>> {
    self.state.with_write(|state| {
      // Pekko parity: when the queue is already at capacity, evict the
      // oldest element and surface it so the caller can forward it to the
      // dead-letter destination instead of silently dropping it.
      let evicted = if state.len() >= capacity {
        match state.poll() {
          | Ok(item) => Some(item),
          // Under the same write lock, `len >= capacity >= 1` guarantees
          // at least one element is present, so `poll` cannot return
          // `Empty` here. We still fall through with `None` defensively;
          // other error variants would be pathological for a VecDeque
          // backend and are handled identically by the caller (no
          // eviction surfaced).
          | Err(_) => None,
        }
      } else {
        None
      };
      state.offer(message)?;
      Ok(match evicted {
        | Some(item) => DropOldestOutcome::Evicted(item),
        | None => DropOldestOutcome::Accepted,
      })
    })
  }

  pub(crate) fn poll(&self) -> Result<T, QueueError<T>> {
    self.state.with_write(|state| state.poll())
  }

  /// Returns the current queue size without acquiring a lock.
  #[must_use]
  pub(crate) fn len(&self) -> usize {
    self.state.with_read(|state| state.len())
  }
}

const fn map_overflow(strategy: MailboxOverflowStrategy) -> OverflowPolicy {
  match strategy {
    | MailboxOverflowStrategy::DropNewest => OverflowPolicy::DropNewest,
    | MailboxOverflowStrategy::DropOldest => OverflowPolicy::DropOldest,
    | MailboxOverflowStrategy::Grow => OverflowPolicy::Grow,
  }
}
