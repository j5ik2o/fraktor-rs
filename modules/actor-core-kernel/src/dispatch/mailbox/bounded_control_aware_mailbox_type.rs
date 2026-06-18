//! Factory for bounded control-aware message queues.

#[cfg(test)]
#[path = "bounded_control_aware_mailbox_type_test.rs"]
mod tests;

use alloc::boxed::Box;
use core::{num::NonZeroUsize, time::Duration};

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
  capacity:     NonZeroUsize,
  overflow:     MailboxOverflowStrategy,
  push_timeout: Option<Duration>,
}

impl BoundedControlAwareMailboxType {
  /// Creates a new bounded control-aware mailbox type factory.
  #[must_use]
  pub const fn new(capacity: NonZeroUsize, overflow: MailboxOverflowStrategy) -> Self {
    Self { capacity, overflow, push_timeout: None }
  }

  /// Creates a bounded control-aware mailbox type factory with Pekko-style push timeout semantics.
  #[must_use]
  pub const fn new_with_push_timeout(
    capacity: NonZeroUsize,
    overflow: MailboxOverflowStrategy,
    push_timeout: Duration,
  ) -> Self {
    Self { capacity, overflow, push_timeout: Some(push_timeout) }
  }
}

impl MailboxType for BoundedControlAwareMailboxType {
  fn create(&self) -> Box<dyn MessageQueue> {
    match self.push_timeout {
      | Some(push_timeout) => {
        Box::new(BoundedControlAwareMessageQueue::new_with_push_timeout(self.capacity, self.overflow, push_timeout))
      },
      | None => Box::new(BoundedControlAwareMessageQueue::new(self.capacity, self.overflow)),
    }
  }
}
