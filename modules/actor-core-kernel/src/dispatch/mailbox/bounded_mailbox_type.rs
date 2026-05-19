//! Factory for bounded message queues.

#[cfg(test)]
#[path = "bounded_mailbox_type_test.rs"]
mod tests;

use alloc::boxed::Box;
use core::num::NonZeroUsize;

use super::{
  bounded_message_queue::BoundedMessageQueue, mailbox_type::MailboxType, message_queue::MessageQueue,
  overflow_strategy::MailboxOverflowStrategy,
};

/// Produces [`BoundedMessageQueue`] instances with the configured capacity and overflow strategy.
pub struct BoundedMailboxType {
  capacity: NonZeroUsize,
  overflow: MailboxOverflowStrategy,
}

impl BoundedMailboxType {
  /// Creates a new bounded mailbox type factory.
  #[must_use]
  pub const fn new(capacity: NonZeroUsize, overflow: MailboxOverflowStrategy) -> Self {
    Self { capacity, overflow }
  }
}

impl MailboxType for BoundedMailboxType {
  fn create(&self) -> Box<dyn MessageQueue> {
    Box::new(BoundedMessageQueue::new(self.capacity, self.overflow))
  }
}
