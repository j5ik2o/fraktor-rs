//! Bounded priority message queue backed by a binary heap with capacity control.

#[cfg(test)]
mod tests;

use core::num::NonZeroUsize;

use fraktor_utils_core_rs::core::sync::{ArcShared, SharedAccess};

use super::{
  bounded_priority_message_queue_state::BoundedPriorityMessageQueueEntry,
  bounded_priority_message_queue_state_shared::BoundedPriorityMessageQueueStateShared, envelope::Envelope,
  message_queue::MessageQueue, overflow_strategy::MailboxOverflowStrategy,
};
use crate::core::kernel::{
  actor::error::SendError, dispatch::mailbox::message_priority_generator::MessagePriorityGenerator,
};

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
    Self { state_shared, generator, capacity: capacity.get(), overflow }
  }
}

impl MessageQueue for BoundedPriorityMessageQueue {
  fn enqueue(&self, envelope: Envelope) -> Result<(), SendError> {
    let priority = self.generator.priority(envelope.payload());
    let entry = BoundedPriorityMessageQueueEntry::new(priority, envelope);
    self.state_shared.with_write(|state| {
      if state.heap().len() < self.capacity {
        state.heap_mut().push(entry);
        return Ok(());
      }

      match self.overflow {
        | MailboxOverflowStrategy::DropNewest => {
          // Capacity full — drop the incoming envelope.
          Err(SendError::full(entry.into_envelope().into_payload()))
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
    self.state_shared.with_write(|state| state.heap_mut().pop().map(BoundedPriorityMessageQueueEntry::into_envelope))
  }

  fn number_of_messages(&self) -> usize {
    self.state_shared.with_read(|state| state.heap().len())
  }

  fn clean_up(&self) {
    self.state_shared.with_write(|state| state.heap_mut().clear());
  }
}
