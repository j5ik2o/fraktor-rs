//! Factory for unbounded priority message queues.

#[cfg(test)]
mod tests;

use alloc::boxed::Box;

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::{
  mailbox_type::MailboxType, message_priority_generator::MessagePriorityGenerator, message_queue::MessageQueue,
  unbounded_priority_message_queue::UnboundedPriorityMessageQueue,
};

/// Produces [`UnboundedPriorityMessageQueue`] instances.
///
/// This factory is selected by [`Mailboxes`](super::Mailboxes) when a priority
/// generator is present in the mailbox configuration and the policy is unbounded.
pub struct UnboundedPriorityMailboxType {
  generator: ArcShared<dyn MessagePriorityGenerator>,
}

impl UnboundedPriorityMailboxType {
  /// Creates a new unbounded priority mailbox type factory.
  #[must_use]
  pub fn new(generator: ArcShared<dyn MessagePriorityGenerator>) -> Self {
    Self { generator }
  }
}

impl MailboxType for UnboundedPriorityMailboxType {
  fn create(&self) -> Box<dyn MessageQueue> {
    Box::new(UnboundedPriorityMessageQueue::new(self.generator.clone()))
  }
}
