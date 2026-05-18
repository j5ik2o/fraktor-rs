//! Handles for interacting with queue producers/consumers.

use core::cmp;

use fraktor_utils_core_rs::{
  collections::queue::{OfferOutcome, OverflowPolicy, QueueError, SyncQueue, backend::VecDequeBackend},
  sync::{SharedAccess, SharedLock},
};

use super::{
  drop_oldest_error::DropOldestError,
  drop_oldest_outcome::DropOldestOutcome,
  mailbox_queue_state::{QueueState, queue_state_shared},
};
use crate::dispatch::mailbox::{
  capacity::MailboxCapacity, overflow_strategy::MailboxOverflowStrategy, policy::MailboxPolicy,
};

const DEFAULT_QUEUE_CAPACITY: usize = 16;

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
  ) -> Result<DropOldestOutcome<T>, DropOldestError<T>> {
    self.state.with_write(|state| {
      // Pekko 互換: キューが既に容量上限に達している場合、最古の要素を evict
      // して呼び出し元に返却し、サイレントに破棄せず dead-letter 宛先へ転送
      // できるようにする。
      let evicted = if state.len() >= capacity {
        match state.poll() {
          | Ok(item) => Some(item),
          // 同じ write lock 下で `len >= capacity >= 1` が保証されているため、
          // ここで `poll` が `Empty` を返すことはない。防御的に `None` で fall
          // through する。他のエラーバリアントは VecDeque backend では病的
          // ケースであり、呼び出し元は同様に扱う (eviction を surface しない)。
          | Err(_) => None,
        }
      } else {
        None
      };
      match state.offer(message) {
        | Ok(_) => Ok(match evicted {
          | Some(item) => DropOldestOutcome::Evicted(item),
          | None => DropOldestOutcome::Accepted,
        }),
        // `poll` で既に evict 済みの要素があった場合、`offer` が失敗しても
        // evicted を失わず呼び出し元が dead-letter へ転送できるよう、
        // `DropOldestError` 経由で返却する。`poll` で queue 内に空きを作った
        // 直後なので `Closed` 以外の `offer` 失敗は VecDeque backend では
        // 発生しない (同一 write lock 内で状態は変化しないため)。
        | Err(error) => Err(DropOldestError { error, evicted }),
      }
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
