//! Unbounded priority message queue backed by shared mailbox state.

#[cfg(test)]
mod tests;

use fraktor_utils_core_rs::core::sync::{ArcShared, SharedAccess};

use super::{
  enqueue_outcome::EnqueueOutcome, envelope::Envelope, message_queue::MessageQueue,
  unbounded_priority_message_queue_state::UnboundedPriorityMessageQueueEntry,
  unbounded_priority_message_queue_state_shared::UnboundedPriorityMessageQueueStateShared,
};
use crate::core::kernel::{
  actor::error::SendError, dispatch::mailbox::message_priority_generator::MessagePriorityGenerator,
};

/// Unbounded message queue that dequeues envelopes in priority order.
///
/// Inspired by Pekko's `UnboundedPriorityMailbox`. A [`MessagePriorityGenerator`]
/// assigns an integer priority to each message; lower values are dequeued first.
pub struct UnboundedPriorityMessageQueue {
  state_shared: UnboundedPriorityMessageQueueStateShared,
  generator:    ArcShared<dyn MessagePriorityGenerator>,
}

impl UnboundedPriorityMessageQueue {
  /// Creates a new unbounded priority message queue.
  #[must_use]
  pub fn new(
    generator: ArcShared<dyn MessagePriorityGenerator>,
    state_shared: UnboundedPriorityMessageQueueStateShared,
  ) -> Self {
    Self { state_shared, generator }
  }
}

impl MessageQueue for UnboundedPriorityMessageQueue {
  fn enqueue(&self, envelope: Envelope) -> Result<EnqueueOutcome, SendError> {
    let priority = self.generator.priority(envelope.payload());
    self
      .state_shared
      .with_write(|state| state.heap_mut().push(UnboundedPriorityMessageQueueEntry::new(priority, envelope)));
    Ok(EnqueueOutcome::Accepted)
  }

  fn dequeue(&self) -> Option<Envelope> {
    self.state_shared.with_write(|state| state.heap_mut().pop().map(UnboundedPriorityMessageQueueEntry::into_envelope))
  }

  fn number_of_messages(&self) -> usize {
    self.state_shared.with_read(|state| state.heap().len())
  }

  fn clean_up(&self) {
    self.state_shared.with_write(|state| state.heap_mut().clear());
  }
}
