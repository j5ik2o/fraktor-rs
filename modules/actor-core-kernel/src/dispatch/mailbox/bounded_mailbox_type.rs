//! Factory for bounded message queues.

#[cfg(test)]
#[path = "bounded_mailbox_type_test.rs"]
mod tests;

use alloc::boxed::Box;
use core::{num::NonZeroUsize, time::Duration};

use super::{
  bounded_message_queue::BoundedMessageQueue, mailbox_type::MailboxType, message_queue::MessageQueue,
  overflow_strategy::MailboxOverflowStrategy,
};

/// Produces [`BoundedMessageQueue`] instances with the configured capacity and overflow strategy.
pub struct BoundedMailboxType {
  capacity:     NonZeroUsize,
  overflow:     MailboxOverflowStrategy,
  push_timeout: Option<Duration>,
}

impl BoundedMailboxType {
  /// Creates a new bounded mailbox type factory.
  #[must_use]
  pub const fn new(capacity: NonZeroUsize, overflow: MailboxOverflowStrategy) -> Self {
    Self { capacity, overflow, push_timeout: None }
  }

  /// Creates a bounded mailbox type factory with push-timeout reporting.
  #[must_use]
  pub const fn new_with_push_timeout(
    capacity: NonZeroUsize,
    overflow: MailboxOverflowStrategy,
    push_timeout: Duration,
  ) -> Self {
    Self { capacity, overflow, push_timeout: Some(push_timeout) }
  }
}

impl MailboxType for BoundedMailboxType {
  fn create(&self) -> Box<dyn MessageQueue> {
    match self.push_timeout {
      | Some(push_timeout) => {
        Box::new(BoundedMessageQueue::new_with_push_timeout(self.capacity, self.overflow, push_timeout))
      },
      | None => Box::new(BoundedMessageQueue::new(self.capacity, self.overflow)),
    }
  }
}
