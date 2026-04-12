//! Factory for bounded stable-priority message queues.

#[cfg(test)]
mod tests;

use alloc::boxed::Box;
use core::num::NonZeroUsize;

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::{
  bounded_stable_priority_message_queue::BoundedStablePriorityMessageQueue,
  bounded_stable_priority_message_queue_state::BoundedStablePriorityMessageQueueState,
  bounded_stable_priority_message_queue_state_shared::BoundedStablePriorityMessageQueueStateShared,
  mailbox_type::MailboxType, message_priority_generator::MessagePriorityGenerator, message_queue::MessageQueue,
  overflow_strategy::MailboxOverflowStrategy,
};

/// Produces [`BoundedStablePriorityMessageQueue`] instances with the configured
/// capacity, overflow strategy, and priority generator.
pub struct BoundedStablePriorityMailboxType {
  generator: ArcShared<dyn MessagePriorityGenerator>,
  capacity:  NonZeroUsize,
  overflow:  MailboxOverflowStrategy,
}

impl BoundedStablePriorityMailboxType {
  /// Creates a new bounded stable-priority mailbox type factory.
  #[must_use]
  pub fn new(
    generator: ArcShared<dyn MessagePriorityGenerator>,
    capacity: NonZeroUsize,
    overflow: MailboxOverflowStrategy,
  ) -> Self {
    Self { generator, capacity, overflow }
  }
}

impl MailboxType for BoundedStablePriorityMailboxType {
  fn create(&self) -> Box<dyn MessageQueue> {
    let state_shared =
      BoundedStablePriorityMessageQueueStateShared::new(BoundedStablePriorityMessageQueueState::with_capacity(
        self.capacity,
      ));
    Box::new(BoundedStablePriorityMessageQueue::new(self.generator.clone(), state_shared, self.capacity, self.overflow))
  }
}
