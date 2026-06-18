//! Bounded stable-priority message queue backed by a binary heap with capacity control.
//!
//! Unlike [`super::BoundedPriorityMessageQueue`], envelopes with equal
//! priority are dequeued in FIFO (insertion) order.

#[cfg(test)]
#[path = "bounded_stable_priority_message_queue_test.rs"]
mod tests;

use core::{num::NonZeroUsize, time::Duration};

use fraktor_utils_core_rs::sync::{ArcShared, SharedAccess};

use super::{
  bounded_stable_priority_message_queue_state_shared::BoundedStablePriorityMessageQueueStateShared,
  enqueue_error::EnqueueError, enqueue_outcome::EnqueueOutcome, envelope::Envelope, mailbox_clock::MailboxClock,
  message_queue::MessageQueue, overflow_strategy::MailboxOverflowStrategy, push_timeout,
  stable_priority_entry::StablePriorityEntry,
};
use crate::dispatch::mailbox::message_priority_generator::MessagePriorityGenerator;

/// Bounded message queue that dequeues in priority order with stable
/// (FIFO) ordering among envelopes of equal priority.
///
/// Inspired by Pekko's `BoundedStablePriorityMailbox`. A
/// [`MessagePriorityGenerator`] assigns an integer priority to each message;
/// lower values are dequeued first. When the queue reaches capacity, the
/// configured [`MailboxOverflowStrategy`] determines the behaviour.
pub struct BoundedStablePriorityMessageQueue {
  state_shared: BoundedStablePriorityMessageQueueStateShared,
  generator:    ArcShared<dyn MessagePriorityGenerator>,
  capacity:     usize,
  overflow:     MailboxOverflowStrategy,
  push_timeout: Option<Duration>,
}

impl BoundedStablePriorityMessageQueue {
  /// Creates a new bounded stable-priority message queue.
  #[must_use]
  pub fn new(
    generator: ArcShared<dyn MessagePriorityGenerator>,
    state_shared: BoundedStablePriorityMessageQueueStateShared,
    capacity: NonZeroUsize,
    overflow: MailboxOverflowStrategy,
  ) -> Self {
    Self { state_shared, generator, capacity: capacity.get(), overflow, push_timeout: None }
  }

  /// Creates a bounded stable-priority message queue with Pekko-style push timeout semantics.
  #[must_use]
  pub fn new_with_push_timeout(
    generator: ArcShared<dyn MessagePriorityGenerator>,
    state_shared: BoundedStablePriorityMessageQueueStateShared,
    capacity: NonZeroUsize,
    overflow: MailboxOverflowStrategy,
    push_timeout: Duration,
  ) -> Self {
    let mut queue = Self::new(generator, state_shared, capacity, overflow);
    queue.push_timeout = Some(push_timeout);
    queue
  }
}

impl MessageQueue for BoundedStablePriorityMessageQueue {
  fn enqueue(&self, envelope: Envelope) -> Result<EnqueueOutcome, EnqueueError> {
    self.enqueue_with_mailbox_clock(envelope, None)
  }

  fn enqueue_with_mailbox_clock(
    &self,
    envelope: Envelope,
    clock: Option<&MailboxClock>,
  ) -> Result<EnqueueOutcome, EnqueueError> {
    let priority = self.generator.priority(envelope.payload());
    if let (Some(timeout), Some(clock)) = (self.push_timeout, clock) {
      return self.enqueue_with_push_timeout(envelope, priority, timeout, clock);
    }
    self.state_shared.with_write(|state| {
      let sequence = state.next_sequence();
      let entry = StablePriorityEntry { priority, sequence, envelope };

      if state.heap().len() < self.capacity {
        state.heap_mut().push(entry);
        return Ok(EnqueueOutcome::Accepted);
      }

      match self.overflow {
        | MailboxOverflowStrategy::DropNewest => {
          // Pekko 互換: 容量上限に達したため到着 envelope を拒否する。
          // mailbox 層が `EnqueueOutcome::Rejected` を DeadLetters へ転送するので
          // ここでは成功として返す (Pekko `BoundedStablePriorityMailbox` 相当)。
          Ok(EnqueueOutcome::Rejected(entry.envelope))
        },
        | MailboxOverflowStrategy::DropOldest => {
          // Pekko 互換: キュー先頭（次にデキューされる最高優先度メッセージ、同 priority 時は
          // 最小 sequence = 最古挿入）を削除し、evict した envelope を
          // `EnqueueOutcome::Evicted` として通知する。呼び出し元は DeadLetter に転送する。
          let evicted = state.heap_mut().pop().map(|entry| entry.envelope);
          state.heap_mut().push(entry);
          match evicted {
            | Some(envelope) => Ok(EnqueueOutcome::Evicted(envelope)),
            // ヒープが満杯であるにもかかわらず `pop` が `None` を返すケースは
            // `len >= capacity >= 1` を write lock 下で保証しているため発生しない。
            // 防御的に `Accepted` を返す。
            | None => Ok(EnqueueOutcome::Accepted),
          }
        },
        | MailboxOverflowStrategy::Grow => {
          // 容量境界を無視して拡張する。
          state.heap_mut().push(entry);
          Ok(EnqueueOutcome::Accepted)
        },
      }
    })
  }

  fn dequeue(&self) -> Option<Envelope> {
    self.state_shared.with_write(|state| state.heap_mut().pop().map(|entry| entry.envelope))
  }

  fn number_of_messages(&self) -> usize {
    self.state_shared.with_read(|state| state.heap().len())
  }

  fn clean_up(&self) {
    self.state_shared.with_write(|state| state.heap_mut().clear());
  }
}

impl BoundedStablePriorityMessageQueue {
  fn enqueue_with_push_timeout(
    &self,
    mut envelope: Envelope,
    priority: i32,
    timeout: Duration,
    clock: &MailboxClock,
  ) -> Result<EnqueueOutcome, EnqueueError> {
    let deadline = push_timeout::push_timeout_deadline(clock, timeout);
    loop {
      let result = self.state_shared.with_write(|state| {
        if state.heap().len() < self.capacity {
          let sequence = state.next_sequence();
          state.heap_mut().push(StablePriorityEntry { priority, sequence, envelope });
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
}
