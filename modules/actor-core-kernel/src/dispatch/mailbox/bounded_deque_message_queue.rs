//! Bounded deque-based message queue with capacity enforcement and front insertion.

#[cfg(test)]
#[path = "bounded_deque_message_queue_test.rs"]
mod tests;

use alloc::collections::VecDeque;
use core::{num::NonZeroUsize, time::Duration};

use fraktor_utils_core_rs::sync::{DefaultMutex, SharedAccess, SharedLock};

use super::{
  deque_message_queue::DequeMessageQueue, enqueue_error::EnqueueError, enqueue_outcome::EnqueueOutcome,
  envelope::Envelope, mailbox_clock::MailboxClock, message_queue::MessageQueue,
  overflow_strategy::MailboxOverflowStrategy, push_timeout,
};
use crate::actor::error::SendError;

/// Bounded deque-based message queue with fixed capacity and configurable overflow behaviour.
///
/// Combines the semantics of Pekko's `BoundedDequeBasedMailbox` (Mailbox.scala:844):
/// front insertion via [`DequeMessageQueue::enqueue_first`] is available, and back
/// insertion via [`MessageQueue::enqueue`] enforces `capacity` according to the chosen
/// [`MailboxOverflowStrategy`].
pub struct BoundedDequeMessageQueue {
  inner:        SharedLock<VecDeque<Envelope>>,
  capacity:     usize,
  overflow:     MailboxOverflowStrategy,
  push_timeout: Option<Duration>,
}

impl BoundedDequeMessageQueue {
  /// Creates a new bounded deque message queue.
  #[must_use]
  pub fn new(capacity: NonZeroUsize, overflow: MailboxOverflowStrategy) -> Self {
    let capacity_value = capacity.get();
    Self {
      inner: SharedLock::new_with_driver::<DefaultMutex<_>>(VecDeque::with_capacity(capacity_value)),
      capacity: capacity_value,
      overflow,
      push_timeout: None,
    }
  }

  /// Creates a bounded deque message queue with Pekko-style push timeout semantics.
  #[must_use]
  pub fn new_with_push_timeout(
    capacity: NonZeroUsize,
    overflow: MailboxOverflowStrategy,
    push_timeout: Duration,
  ) -> Self {
    let mut queue = Self::new(capacity, overflow);
    queue.push_timeout = Some(push_timeout);
    queue
  }
}

impl MessageQueue for BoundedDequeMessageQueue {
  fn enqueue(&self, envelope: Envelope) -> Result<EnqueueOutcome, EnqueueError> {
    self.enqueue_with_mailbox_clock(envelope, None)
  }

  fn enqueue_with_mailbox_clock(
    &self,
    envelope: Envelope,
    clock: Option<&MailboxClock>,
  ) -> Result<EnqueueOutcome, EnqueueError> {
    if let (Some(timeout), Some(clock)) = (self.push_timeout, clock) {
      return self.enqueue_back_with_push_timeout(envelope, timeout, clock);
    }
    self.inner.with_write(|inner| match self.overflow {
      | MailboxOverflowStrategy::Grow => {
        inner.push_back(envelope);
        Ok(EnqueueOutcome::Accepted)
      },
      | MailboxOverflowStrategy::DropNewest => {
        if inner.len() >= self.capacity {
          Ok(EnqueueOutcome::Rejected(envelope))
        } else {
          inner.push_back(envelope);
          Ok(EnqueueOutcome::Accepted)
        }
      },
      | MailboxOverflowStrategy::DropOldest => {
        if inner.len() >= self.capacity {
          // `capacity >= 1` (NonZeroUsize) かつ `len >= capacity` から `pop_front()` は必ず
          // `Some` を返すが、`clippy::expect_used` を避けるため明示的な match で扱う。
          match inner.pop_front() {
            | Some(evicted) => {
              inner.push_back(envelope);
              Ok(EnqueueOutcome::Evicted(evicted))
            },
            | None => {
              inner.push_back(envelope);
              Ok(EnqueueOutcome::Accepted)
            },
          }
        } else {
          inner.push_back(envelope);
          Ok(EnqueueOutcome::Accepted)
        }
      },
    })
  }

  fn dequeue(&self) -> Option<Envelope> {
    self.inner.with_write(|inner| inner.pop_front())
  }

  fn number_of_messages(&self) -> usize {
    self.inner.with_read(|inner| inner.len())
  }

  fn clean_up(&self) {
    self.inner.with_write(|inner| inner.clear());
  }

  fn as_deque(&self) -> Option<&dyn DequeMessageQueue> {
    Some(self)
  }
}

impl DequeMessageQueue for BoundedDequeMessageQueue {
  fn enqueue_first(&self, envelope: Envelope) -> Result<(), SendError> {
    self.enqueue_first_with_mailbox_clock(envelope, None)
  }

  fn enqueue_first_with_mailbox_clock(
    &self,
    envelope: Envelope,
    clock: Option<&MailboxClock>,
  ) -> Result<(), SendError> {
    if let (Some(timeout), Some(clock)) = (self.push_timeout, clock) {
      return self.enqueue_front_with_push_timeout(envelope, timeout, clock);
    }
    self.inner.with_write(|inner| match self.overflow {
      | MailboxOverflowStrategy::Grow => {
        inner.push_front(envelope);
        Ok(())
      },
      // DropNewest / DropOldest は capacity 超過時にいずれも evict せず Reject する。
      // DropOldest で front を evict すると push_front 直後に同じ envelope を捨てる矛盾が生じる
      // (design Decision 2-c)。spec Requirement 1 Scenario "Decision 2-c" を参照。
      | MailboxOverflowStrategy::DropNewest | MailboxOverflowStrategy::DropOldest => {
        if inner.len() >= self.capacity {
          Err(SendError::full(envelope.into_payload()))
        } else {
          inner.push_front(envelope);
          Ok(())
        }
      },
    })
  }
}

impl BoundedDequeMessageQueue {
  fn enqueue_back_with_push_timeout(
    &self,
    mut envelope: Envelope,
    timeout: Duration,
    clock: &MailboxClock,
  ) -> Result<EnqueueOutcome, EnqueueError> {
    let deadline = push_timeout::push_timeout_deadline(clock, timeout);
    loop {
      let result = self.inner.with_write(|inner| {
        if inner.len() < self.capacity {
          inner.push_back(envelope);
          Ok(EnqueueOutcome::Accepted)
        } else {
          Ok(EnqueueOutcome::Rejected(envelope))
        }
      });
      match result {
        | Ok(EnqueueOutcome::Accepted) => return Ok(EnqueueOutcome::Accepted),
        | Ok(EnqueueOutcome::Rejected(rejected)) => {
          envelope = rejected;
          if !push_timeout::should_retry_after_full(clock, deadline) {
            return Err(push_timeout::enqueue_timeout(envelope));
          }
          push_timeout::spin_before_push_timeout_retry();
        },
        | Ok(EnqueueOutcome::Evicted(evicted)) => return Ok(EnqueueOutcome::Evicted(evicted)),
        | Err(error) => return Err(error),
      }
    }
  }

  fn enqueue_front_with_push_timeout(
    &self,
    mut envelope: Envelope,
    timeout: Duration,
    clock: &MailboxClock,
  ) -> Result<(), SendError> {
    let deadline = push_timeout::push_timeout_deadline(clock, timeout);
    loop {
      let result = self.inner.with_write(|inner| {
        if inner.len() < self.capacity {
          inner.push_front(envelope);
          Ok(())
        } else {
          Err(envelope)
        }
      });
      match result {
        | Ok(()) => return Ok(()),
        | Err(rejected) => {
          envelope = rejected;
          if !push_timeout::should_retry_after_full(clock, deadline) {
            return Err(push_timeout::send_timeout(envelope));
          }
          push_timeout::spin_before_push_timeout_retry();
        },
      }
    }
  }
}
