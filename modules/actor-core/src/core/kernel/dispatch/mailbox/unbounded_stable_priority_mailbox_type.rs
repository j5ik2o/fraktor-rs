//! Factory for unbounded stable-priority message queues.

#[cfg(test)]
mod tests;

use alloc::boxed::Box;

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::{
  mailbox_type::MailboxType, message_priority_generator::MessagePriorityGenerator, message_queue::MessageQueue,
  unbounded_stable_priority_message_queue::UnboundedStablePriorityMessageQueue,
};

/// Produces [`UnboundedStablePriorityMessageQueue`] instances.
///
/// This factory is selected by [`Mailboxes`](super::Mailboxes) when a priority
/// generator is present, stable ordering is enabled, and the policy is unbounded.
pub struct UnboundedStablePriorityMailboxType {
  generator: ArcShared<dyn MessagePriorityGenerator>,
}

impl UnboundedStablePriorityMailboxType {
  /// Creates a new unbounded stable-priority mailbox type factory.
  #[must_use]
  pub fn new(generator: ArcShared<dyn MessagePriorityGenerator>) -> Self {
    Self { generator }
  }
}

impl MailboxType for UnboundedStablePriorityMailboxType {
  fn create(&self) -> Box<dyn MessageQueue> {
    Box::new(UnboundedStablePriorityMessageQueue::new(self.generator.clone()))
  }
}
