//! Mailbox policy describing capacity and overflow behaviour.

use core::num::NonZeroUsize;

use crate::{MailboxCapacity, MailboxOverflowStrategy};

/// Configuration applied to each mailbox instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MailboxPolicy {
  capacity:         MailboxCapacity,
  overflow:         MailboxOverflowStrategy,
  throughput_limit: Option<NonZeroUsize>,
}

impl MailboxPolicy {
  /// Creates a new policy from its components.
  #[must_use]
  pub const fn new(
    capacity: MailboxCapacity,
    overflow: MailboxOverflowStrategy,
    throughput_limit: Option<NonZeroUsize>,
  ) -> Self {
    Self { capacity, overflow, throughput_limit }
  }

  /// Creates a bounded mailbox policy with the provided capacity and overflow strategy.
  #[must_use]
  pub const fn bounded(
    capacity: NonZeroUsize,
    overflow: MailboxOverflowStrategy,
    throughput_limit: Option<NonZeroUsize>,
  ) -> Self {
    Self::new(MailboxCapacity::Bounded { capacity }, overflow, throughput_limit)
  }

  /// Creates an unbounded mailbox policy.
  #[must_use]
  pub const fn unbounded(throughput_limit: Option<NonZeroUsize>) -> Self {
    Self::new(MailboxCapacity::Unbounded, MailboxOverflowStrategy::DropOldest, throughput_limit)
  }

  /// Returns the configured capacity.
  #[must_use]
  pub const fn capacity(&self) -> MailboxCapacity {
    self.capacity
  }

  /// Returns the overflow strategy.
  #[must_use]
  pub const fn overflow(&self) -> MailboxOverflowStrategy {
    self.overflow
  }

  /// Returns the throughput limit, if any.
  #[must_use]
  pub const fn throughput_limit(&self) -> Option<NonZeroUsize> {
    self.throughput_limit
  }

  /// Returns a copy of the policy with a different throughput limit.
  #[must_use]
  pub const fn with_throughput_limit(self, limit: Option<NonZeroUsize>) -> Self {
    Self { throughput_limit: limit, ..self }
  }

  /// Returns a copy of the policy with a different overflow strategy.
  #[must_use]
  pub const fn with_overflow(self, overflow: MailboxOverflowStrategy) -> Self {
    Self { overflow, ..self }
  }

  /// Returns a copy of the policy with a different capacity configuration.
  #[must_use]
  pub const fn with_capacity(self, capacity: MailboxCapacity) -> Self {
    Self { capacity, ..self }
  }
}

#[cfg(test)]
mod tests;
