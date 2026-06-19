//! Bounded priority message queue backed by a binary heap with capacity control.

#[cfg(test)]
#[path = "bounded_priority_message_queue_test.rs"]
mod tests;

use core::{num::NonZeroUsize, time::Duration};

use fraktor_utils_core_rs::sync::{ArcShared, SharedAccess};

use super::{
  bounded_priority_message_queue_state::BoundedPriorityMessageQueueEntry,
  bounded_priority_message_queue_state_shared::BoundedPriorityMessageQueueStateShared, enqueue_error::EnqueueError,
  enqueue_outcome::EnqueueOutcome, envelope::Envelope, mailbox_clock::MailboxClock, message_queue::MessageQueue,
  overflow_strategy::MailboxOverflowStrategy, push_timeout,
};
use crate::dispatch::mailbox::message_priority_generator::MessagePriorityGenerator;

/// Bounded message queue that dequeues envelopes in priority order.
///
/// Inspired by Pekko's `BoundedPriorityMailbox`. A [`MessagePriorityGenerator`]
/// assigns an integer priority to each message; lower values are dequeued first.
/// When the queue reaches capacity, the configured [`MailboxOverflowStrategy`]
/// determines the behaviour.
pub struct BoundedPriorityMessageQueue {
  state_shared: BoundedPriorityMessageQueueStateShared,
  generator:    ArcShared<dyn MessagePriorityGenerator>,
  capacity:     usize,
  overflow:     MailboxOverflowStrategy,
  push_timeout: Option<Duration>,
}

impl BoundedPriorityMessageQueue {
  /// Creates a new bounded priority message queue.
  #[must_use]
  pub fn new(
    generator: ArcShared<dyn MessagePriorityGenerator>,
    state_shared: BoundedPriorityMessageQueueStateShared,
    capacity: NonZeroUsize,
    overflow: MailboxOverflowStrategy,
  ) -> Self {
    Self { state_shared, generator, capacity: capacity.get(), overflow, push_timeout: None }
  }

  /// Creates a bounded priority message queue with push-timeout reporting.
  #[must_use]
  pub fn new_with_push_timeout(
    generator: ArcShared<dyn MessagePriorityGenerator>,
    state_shared: BoundedPriorityMessageQueueStateShared,
    capacity: NonZeroUsize,
    overflow: MailboxOverflowStrategy,
    push_timeout: Duration,
  ) -> Self {
    let mut queue = Self::new(generator, state_shared, capacity, overflow);
    queue.push_timeout = Some(push_timeout);
    queue
  }
}

impl MessageQueue for BoundedPriorityMessageQueue {
  fn enqueue(&self, envelope: Envelope) -> Result<EnqueueOutcome, EnqueueError> {
    self.enqueue_with_mailbox_clock(envelope, None)
  }

  fn enqueue_with_mailbox_clock(
    &self,
    envelope: Envelope,
    clock: Option<&MailboxClock>,
  ) -> Result<EnqueueOutcome, EnqueueError> {
    let priority = self.generator.priority(envelope.payload());
    let entry = BoundedPriorityMessageQueueEntry::new(priority, envelope);
    if self.overflow != MailboxOverflowStrategy::Grow
      && let (Some(timeout), Some(clock)) = (self.push_timeout, clock)
    {
      return self.enqueue_entry_with_push_timeout(entry, timeout, clock);
    }
    self.state_shared.with_write(|state| {
      if state.heap().len() < self.capacity {
        state.heap_mut().push(entry);
        return Ok(EnqueueOutcome::Accepted);
      }

      match self.overflow {
        | MailboxOverflowStrategy::DropNewest => {
          // Pekko 互換: 容量上限に達したため到着 envelope を拒否する。
          // mailbox 層が `EnqueueOutcome::Rejected` を DeadLetters へ転送する
          // ので、ここでは成功として返す (Pekko `BoundedPriorityMailbox` 相当)。
          Ok(EnqueueOutcome::Rejected(entry.into_envelope()))
        },
        | MailboxOverflowStrategy::DropOldest => {
          // Pekko 互換: キュー先頭（次にデキューされる最高優先度メッセージ）を削除し、
          // evict した envelope を `EnqueueOutcome::Evicted` として呼び出し元
          // (mailbox 層) に通知する。呼び出し元は DeadLetter に転送する。
          let evicted = state.heap_mut().pop().map(BoundedPriorityMessageQueueEntry::into_envelope);
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
    self.state_shared.with_write(|state| state.heap_mut().pop().map(BoundedPriorityMessageQueueEntry::into_envelope))
  }

  fn number_of_messages(&self) -> usize {
    self.state_shared.with_read(|state| state.heap().len())
  }

  fn clean_up(&self) {
    self.state_shared.with_write(|state| state.heap_mut().clear());
  }
}

impl BoundedPriorityMessageQueue {
  fn enqueue_entry_with_push_timeout(
    &self,
    entry: BoundedPriorityMessageQueueEntry,
    _timeout: Duration,
    _clock: &MailboxClock,
  ) -> Result<EnqueueOutcome, EnqueueError> {
    let rejected = self.state_shared.with_write(|state| {
      if state.heap().len() < self.capacity {
        state.heap_mut().push(entry);
        None
      } else {
        Some(entry)
      }
    });
    match rejected {
      | None => Ok(EnqueueOutcome::Accepted),
      | Some(rejected) => Err(push_timeout::enqueue_timeout(rejected.into_envelope())),
    }
  }
}
