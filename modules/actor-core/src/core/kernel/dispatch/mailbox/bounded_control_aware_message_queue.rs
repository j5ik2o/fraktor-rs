//! Bounded control-aware message queue with dual-queue prioritisation and capacity enforcement.

#[cfg(test)]
mod tests;

use alloc::collections::VecDeque;
use core::{cmp::min, num::NonZeroUsize};

use fraktor_utils_core_rs::core::sync::{DefaultMutex, SharedAccess, SharedLock};

use super::{
  enqueue_error::EnqueueError, enqueue_outcome::EnqueueOutcome, envelope::Envelope, message_queue::MessageQueue,
  overflow_strategy::MailboxOverflowStrategy,
};

/// Initial capacity hint for each backing deque.
///
/// The combined length of the two queues is bounded by `capacity`, so allocating the full
/// `capacity` for each queue would reserve 2× the memory that can ever be used
/// simultaneously. We mirror
/// [`UnboundedControlAwareMessageQueue`](super::UnboundedControlAwareMessageQueue)'s fixed 16-slot
/// hint (clamped to `capacity`) and let `VecDeque` grow on demand.
const DEFAULT_CAPACITY_HINT: usize = 16;

/// Bounded message queue that prioritises control messages and enforces a combined capacity.
///
/// Implements Pekko's `BoundedControlAwareMailbox` (Mailbox.scala:931) semantics: envelopes
/// whose payload is marked as control are routed to a dedicated control queue that is drained
/// before the normal queue. The total length across both queues is bounded by `capacity`; the
/// chosen [`MailboxOverflowStrategy`] decides overflow behaviour. `DropOldest` always evicts
/// from the normal queue so control messages are never silently dropped (design Decision 3).
pub struct BoundedControlAwareMessageQueue {
  inner:    SharedLock<Inner>,
  capacity: usize,
  overflow: MailboxOverflowStrategy,
}

struct Inner {
  control_queue: VecDeque<Envelope>,
  normal_queue:  VecDeque<Envelope>,
}

impl Inner {
  fn total_len(&self) -> usize {
    self.control_queue.len() + self.normal_queue.len()
  }
}

impl BoundedControlAwareMessageQueue {
  /// Creates a new bounded control-aware message queue.
  #[must_use]
  pub fn new(capacity: NonZeroUsize, overflow: MailboxOverflowStrategy) -> Self {
    let capacity_value = capacity.get();
    // control_queue + normal_queue の合計長は capacity で上限されるため、各 queue に capacity
    // を pre-allocate すると 2× の over-allocation になる。UnboundedControlAwareMessageQueue と
    // 同じ固定ヒント (16) に揃え、capacity を下回る場合のみ capacity で clamp する。
    // これにより大容量設定 (capacity ≫ 16) で合計 32 slot に収まり、非常に小さい設定
    // (capacity < 16) でも pre-allocate が capacity 分以下に抑えられる。
    let hint = min(DEFAULT_CAPACITY_HINT, capacity_value);
    Self {
      inner: SharedLock::new_with_driver::<DefaultMutex<_>>(Inner {
        control_queue: VecDeque::with_capacity(hint),
        normal_queue:  VecDeque::with_capacity(hint),
      }),
      capacity: capacity_value,
      overflow,
    }
  }
}

impl MessageQueue for BoundedControlAwareMessageQueue {
  fn enqueue(&self, envelope: Envelope) -> Result<EnqueueOutcome, EnqueueError> {
    self.inner.with_write(|inner| match self.overflow {
      | MailboxOverflowStrategy::Grow => {
        push_into_appropriate_queue(inner, envelope);
        Ok(EnqueueOutcome::Accepted)
      },
      | MailboxOverflowStrategy::DropNewest => {
        if inner.total_len() >= self.capacity {
          Ok(EnqueueOutcome::Rejected(envelope))
        } else {
          push_into_appropriate_queue(inner, envelope);
          Ok(EnqueueOutcome::Accepted)
        }
      },
      | MailboxOverflowStrategy::DropOldest => {
        if inner.total_len() >= self.capacity {
          // design Decision 3: control drop を避けるため常に normal queue の front を
          // evict する。normal queue が空なら到着 envelope (control 含む) を Reject する。
          if let Some(evicted) = inner.normal_queue.pop_front() {
            push_into_appropriate_queue(inner, envelope);
            Ok(EnqueueOutcome::Evicted(evicted))
          } else {
            Ok(EnqueueOutcome::Rejected(envelope))
          }
        } else {
          push_into_appropriate_queue(inner, envelope);
          Ok(EnqueueOutcome::Accepted)
        }
      },
    })
  }

  fn dequeue(&self) -> Option<Envelope> {
    self.inner.with_write(|inner| inner.control_queue.pop_front().or_else(|| inner.normal_queue.pop_front()))
  }

  fn number_of_messages(&self) -> usize {
    self.inner.with_read(Inner::total_len)
  }

  fn clean_up(&self) {
    self.inner.with_write(|inner| {
      inner.control_queue.clear();
      inner.normal_queue.clear();
    });
  }
}

fn push_into_appropriate_queue(inner: &mut Inner, envelope: Envelope) {
  if envelope.payload().is_control() {
    inner.control_queue.push_back(envelope);
  } else {
    inner.normal_queue.push_back(envelope);
  }
}
