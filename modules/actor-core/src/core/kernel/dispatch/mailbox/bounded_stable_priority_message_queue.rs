//! Bounded stable-priority message queue backed by a binary heap with capacity control.
//!
//! Unlike [`super::BoundedPriorityMessageQueue`], envelopes with equal
//! priority are dequeued in FIFO (insertion) order.

#[cfg(test)]
mod tests;

use core::num::NonZeroUsize;

use fraktor_utils_core_rs::core::sync::{ArcShared, SharedAccess};

use super::{
  bounded_stable_priority_message_queue_state_shared::BoundedStablePriorityMessageQueueStateShared, envelope::Envelope,
  message_queue::MessageQueue, overflow_strategy::MailboxOverflowStrategy, stable_priority_entry::StablePriorityEntry,
};
use crate::core::kernel::{
  actor::error::SendError, dispatch::mailbox::message_priority_generator::MessagePriorityGenerator,
};

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
    Self { state_shared, generator, capacity: capacity.get(), overflow }
  }
}

impl MessageQueue for BoundedStablePriorityMessageQueue {
  fn enqueue(&self, envelope: Envelope) -> Result<(), SendError> {
    let priority = self.generator.priority(envelope.payload());
    self.state_shared.with_write(|state| {
      let sequence = state.next_sequence();
      let entry = StablePriorityEntry { priority, sequence, envelope };

      if state.heap().len() < self.capacity {
        state.heap_mut().push(entry);
        return Ok(());
      }

      match self.overflow {
        | MailboxOverflowStrategy::DropNewest => {
          // Capacity full — drop the incoming envelope.
          Err(SendError::full(entry.envelope.into_payload()))
        },
        | MailboxOverflowStrategy::DropOldest => {
          // Pekko 互換: キュー先頭（次にデキューされる最高優先度メッセージ）を削除する
          drop(state.heap_mut().pop());
          state.heap_mut().push(entry);
          Ok(())
        },
        | MailboxOverflowStrategy::Grow => {
          // Ignore the bound and grow.
          state.heap_mut().push(entry);
          Ok(())
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
