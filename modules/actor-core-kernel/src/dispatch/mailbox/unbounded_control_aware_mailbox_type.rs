//! Factory for unbounded control-aware message queues.

#[cfg(test)]
mod tests;

use alloc::boxed::Box;

use super::{
  mailbox_type::MailboxType, message_queue::MessageQueue,
  unbounded_control_aware_message_queue::UnboundedControlAwareMessageQueue,
};

/// Produces [`UnboundedControlAwareMessageQueue`] instances.
///
/// This factory is selected by [`Mailboxes`](super::Mailboxes) when the mailbox
/// requirement declares control-aware semantics and the policy is unbounded.
pub struct UnboundedControlAwareMailboxType;

impl UnboundedControlAwareMailboxType {
  /// Creates a new unbounded control-aware mailbox type factory.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }
}

impl Default for UnboundedControlAwareMailboxType {
  fn default() -> Self {
    Self::new()
  }
}

impl MailboxType for UnboundedControlAwareMailboxType {
  fn create(&self) -> Box<dyn MessageQueue> {
    Box::new(UnboundedControlAwareMessageQueue::new())
  }
}
