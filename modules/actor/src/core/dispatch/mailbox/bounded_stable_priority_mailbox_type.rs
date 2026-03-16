//! Factory for bounded stable-priority message queues.

#[cfg(test)]
mod tests;

use alloc::boxed::Box;
use core::num::NonZeroUsize;

use fraktor_utils_rs::core::sync::ArcShared;

use super::{
  bounded_stable_priority_message_queue::BoundedStablePriorityMessageQueue, mailbox_type::MailboxType,
  message_priority_generator::MessagePriorityGenerator, message_queue::MessageQueue,
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
    Box::new(BoundedStablePriorityMessageQueue::new(self.generator.clone(), self.capacity, self.overflow))
  }
}
