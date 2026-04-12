//! Factory for bounded priority message queues.

#[cfg(test)]
mod tests;

use alloc::boxed::Box;
use core::num::NonZeroUsize;

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::{
  bounded_priority_message_queue::BoundedPriorityMessageQueue,
  bounded_priority_message_queue_state::BoundedPriorityMessageQueueState,
  bounded_priority_message_queue_state_shared::BoundedPriorityMessageQueueStateShared, mailbox_type::MailboxType,
  message_priority_generator::MessagePriorityGenerator, message_queue::MessageQueue,
  overflow_strategy::MailboxOverflowStrategy,
};

/// Produces [`BoundedPriorityMessageQueue`] instances with the configured capacity,
/// overflow strategy, and priority generator.
pub struct BoundedPriorityMailboxType {
  generator: ArcShared<dyn MessagePriorityGenerator>,
  capacity:  NonZeroUsize,
  overflow:  MailboxOverflowStrategy,
}

impl BoundedPriorityMailboxType {
  /// Creates a new bounded priority mailbox type factory.
  #[must_use]
  pub fn new(
    generator: ArcShared<dyn MessagePriorityGenerator>,
    capacity: NonZeroUsize,
    overflow: MailboxOverflowStrategy,
  ) -> Self {
    Self { generator, capacity, overflow }
  }
}

impl MailboxType for BoundedPriorityMailboxType {
  fn create(&self) -> Box<dyn MessageQueue> {
    let state_shared =
      BoundedPriorityMessageQueueStateShared::new(BoundedPriorityMessageQueueState::with_capacity(self.capacity));
    Box::new(BoundedPriorityMessageQueue::new(self.generator.clone(), state_shared, self.capacity, self.overflow))
  }
}
