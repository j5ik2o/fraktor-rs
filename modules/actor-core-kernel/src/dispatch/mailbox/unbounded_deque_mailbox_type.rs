//! Factory for unbounded deque-based message queues.

#[cfg(test)]
mod tests;

use alloc::boxed::Box;

use super::{
  mailbox_type::MailboxType, message_queue::MessageQueue, unbounded_deque_message_queue::UnboundedDequeMessageQueue,
};

/// Produces [`UnboundedDequeMessageQueue`] instances.
///
/// This factory is selected by [`Mailboxes`](super::Mailboxes) when the mailbox
/// requirement declares deque semantics and the policy is unbounded.
pub struct UnboundedDequeMailboxType;

impl UnboundedDequeMailboxType {
  /// Creates a new unbounded deque mailbox type factory.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }
}

impl Default for UnboundedDequeMailboxType {
  fn default() -> Self {
    Self::new()
  }
}

impl MailboxType for UnboundedDequeMailboxType {
  fn create(&self) -> Box<dyn MessageQueue> {
    Box::new(UnboundedDequeMessageQueue::new())
  }
}
