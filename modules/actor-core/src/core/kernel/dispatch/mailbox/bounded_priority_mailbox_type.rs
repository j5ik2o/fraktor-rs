//! Factory for bounded priority message queues.

#[cfg(test)]
mod tests;

use alloc::boxed::Box;
use core::num::NonZeroUsize;

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::{
  bounded_priority_message_queue::BoundedPriorityMessageQueue,
  bounded_priority_message_queue_state_shared::BoundedPriorityMessageQueueState,
  bounded_priority_message_queue_state_shared_factory::BoundedPriorityMessageQueueStateSharedFactory,
  mailbox_type::MailboxType, message_priority_generator::MessagePriorityGenerator, message_queue::MessageQueue,
  overflow_strategy::MailboxOverflowStrategy,
};

/// Produces [`BoundedPriorityMessageQueue`] instances with the configured capacity,
/// overflow strategy, and priority generator.
pub struct BoundedPriorityMailboxType {
  generator:            ArcShared<dyn MessagePriorityGenerator>,
  state_shared_factory: ArcShared<dyn BoundedPriorityMessageQueueStateSharedFactory>,
  capacity:             NonZeroUsize,
  overflow:             MailboxOverflowStrategy,
}

impl BoundedPriorityMailboxType {
  /// Creates a new bounded priority mailbox type factory.
  #[must_use]
  pub fn new(
    generator: ArcShared<dyn MessagePriorityGenerator>,
    state_shared_factory: ArcShared<dyn BoundedPriorityMessageQueueStateSharedFactory>,
    capacity: NonZeroUsize,
    overflow: MailboxOverflowStrategy,
  ) -> Self {
    Self { generator, state_shared_factory, capacity, overflow }
  }
}

impl MailboxType for BoundedPriorityMailboxType {
  fn create(&self) -> Box<dyn MessageQueue> {
    let state_shared = self.state_shared_factory.create_bounded_priority_message_queue_state_shared(
      BoundedPriorityMessageQueueState::with_capacity(self.capacity),
    );
    Box::new(BoundedPriorityMessageQueue::new(self.generator.clone(), state_shared, self.capacity, self.overflow))
  }
}
