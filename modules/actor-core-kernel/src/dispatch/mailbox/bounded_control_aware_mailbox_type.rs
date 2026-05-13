//! Factory for bounded control-aware message queues.

#[cfg(test)]
#[path = "bounded_control_aware_mailbox_type_test.rs"]
mod tests;

use alloc::boxed::Box;
use core::num::NonZeroUsize;

use super::{
  bounded_control_aware_message_queue::BoundedControlAwareMessageQueue, mailbox_type::MailboxType,
  message_queue::MessageQueue, overflow_strategy::MailboxOverflowStrategy,
};

/// Produces [`BoundedControlAwareMessageQueue`] instances with the configured capacity and overflow
/// strategy.
///
/// Selected by [`Mailboxes`](super::Mailboxes) when the mailbox requirement declares
/// control-aware semantics and the policy is bounded (Pekko `BoundedControlAwareMailbox` parity).
pub struct BoundedControlAwareMailboxType {
  capacity: NonZeroUsize,
  overflow: MailboxOverflowStrategy,
}

impl BoundedControlAwareMailboxType {
  /// Creates a new bounded control-aware mailbox type factory.
  #[must_use]
  pub const fn new(capacity: NonZeroUsize, overflow: MailboxOverflowStrategy) -> Self {
    Self { capacity, overflow }
  }
}

impl MailboxType for BoundedControlAwareMailboxType {
  fn create(&self) -> Box<dyn MessageQueue> {
    Box::new(BoundedControlAwareMessageQueue::new(self.capacity, self.overflow))
  }
}
