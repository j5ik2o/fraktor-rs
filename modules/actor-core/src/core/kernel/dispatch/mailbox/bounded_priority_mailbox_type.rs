//! Factory for bounded priority message queues.

#[cfg(test)]
mod tests;

use alloc::boxed::Box;
use core::num::NonZeroUsize;

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::{
  bounded_priority_message_queue::BoundedPriorityMessageQueue, mailbox_type::MailboxType,
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
    Box::new(BoundedPriorityMessageQueue::new(self.generator.clone(), self.capacity, self.overflow))
  }
}
