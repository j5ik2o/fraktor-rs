//! Factory for unbounded message queues.

#[cfg(test)]
mod tests;

use alloc::boxed::Box;

use super::{mailbox_type::MailboxType, message_queue::MessageQueue, unbounded_message_queue::UnboundedMessageQueue};

/// Produces [`UnboundedMessageQueue`] instances.
pub struct UnboundedMailboxType;

impl UnboundedMailboxType {
  /// Creates a new unbounded mailbox type factory.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }
}

impl Default for UnboundedMailboxType {
  fn default() -> Self {
    Self::new()
  }
}

impl MailboxType for UnboundedMailboxType {
  fn create(&self) -> Box<dyn MessageQueue> {
    Box::new(UnboundedMessageQueue::new())
  }
}
