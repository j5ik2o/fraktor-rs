//! Factory for unbounded priority message queues.

#[cfg(test)]
mod tests;

use alloc::boxed::Box;

use fraktor_utils_core_rs::sync::ArcShared;

use super::{
  mailbox_type::MailboxType, message_priority_generator::MessagePriorityGenerator, message_queue::MessageQueue,
  unbounded_priority_message_queue::UnboundedPriorityMessageQueue,
  unbounded_priority_message_queue_state::UnboundedPriorityMessageQueueState,
  unbounded_priority_message_queue_state_shared::UnboundedPriorityMessageQueueStateShared,
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
    let state_shared = UnboundedPriorityMessageQueueStateShared::new(UnboundedPriorityMessageQueueState::new());
    Box::new(UnboundedPriorityMessageQueue::new(self.generator.clone(), state_shared))
  }
}
