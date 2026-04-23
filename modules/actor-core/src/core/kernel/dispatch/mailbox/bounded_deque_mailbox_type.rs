//! Factory for bounded deque-based message queues.

#[cfg(test)]
mod tests;

use alloc::boxed::Box;
use core::num::NonZeroUsize;

use super::{
  bounded_deque_message_queue::BoundedDequeMessageQueue, mailbox_type::MailboxType, message_queue::MessageQueue,
  overflow_strategy::MailboxOverflowStrategy,
};

/// Produces [`BoundedDequeMessageQueue`] instances with the configured capacity and overflow
/// strategy.
///
/// Selected by [`Mailboxes`](super::Mailboxes) when the mailbox requirement declares deque
/// semantics and the policy is bounded (Pekko `BoundedDequeBasedMailbox` parity).
pub struct BoundedDequeMailboxType {
  capacity: NonZeroUsize,
  overflow: MailboxOverflowStrategy,
}

impl BoundedDequeMailboxType {
  /// Creates a new bounded deque mailbox type factory.
  #[must_use]
  pub const fn new(capacity: NonZeroUsize, overflow: MailboxOverflowStrategy) -> Self {
    Self { capacity, overflow }
  }
}

impl MailboxType for BoundedDequeMailboxType {
  fn create(&self) -> Box<dyn MessageQueue> {
    Box::new(BoundedDequeMessageQueue::new(self.capacity, self.overflow))
  }
}
