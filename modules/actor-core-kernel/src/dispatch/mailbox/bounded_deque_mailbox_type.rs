//! Factory for bounded deque-based message queues.

#[cfg(test)]
#[path = "bounded_deque_mailbox_type_test.rs"]
mod tests;

use alloc::boxed::Box;
use core::{num::NonZeroUsize, time::Duration};

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
  capacity:     NonZeroUsize,
  overflow:     MailboxOverflowStrategy,
  push_timeout: Option<Duration>,
}

impl BoundedDequeMailboxType {
  /// Creates a new bounded deque mailbox type factory.
  #[must_use]
  pub const fn new(capacity: NonZeroUsize, overflow: MailboxOverflowStrategy) -> Self {
    Self { capacity, overflow, push_timeout: None }
  }

  /// Creates a bounded deque mailbox type factory with Pekko-style push timeout semantics.
  #[must_use]
  pub const fn new_with_push_timeout(
    capacity: NonZeroUsize,
    overflow: MailboxOverflowStrategy,
    push_timeout: Duration,
  ) -> Self {
    Self { capacity, overflow, push_timeout: Some(push_timeout) }
  }
}

impl MailboxType for BoundedDequeMailboxType {
  fn create(&self) -> Box<dyn MessageQueue> {
    match self.push_timeout {
      | Some(push_timeout) => {
        Box::new(BoundedDequeMessageQueue::new_with_push_timeout(self.capacity, self.overflow, push_timeout))
      },
      | None => Box::new(BoundedDequeMessageQueue::new(self.capacity, self.overflow)),
    }
  }
}
