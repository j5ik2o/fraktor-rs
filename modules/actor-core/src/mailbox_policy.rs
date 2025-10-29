use core::num::NonZeroUsize;

/// Strategy used when a bounded mailbox reaches capacity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MailboxOverflowStrategy {
  /// Drops the newest message offered to the mailbox.
  DropNewest,
  /// Drops the oldest message stored in the mailbox.
  DropOldest,
  /// Dynamically grows the underlying buffer to accommodate more messages.
  Grow,
  /// Blocks the producer until capacity becomes available.
  Block,
}

/// Capacity strategy for the mailbox.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MailboxCapacity {
  /// Capacity is fixed and enforced.
  Bounded { capacity: NonZeroUsize },
  /// Capacity is unbounded and may consume additional heap memory.
  Unbounded,
}

/// Configuration applied to each mailbox instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MailboxPolicy {
  capacity:         MailboxCapacity,
  overflow:         MailboxOverflowStrategy,
  throughput_limit: Option<NonZeroUsize>,
}

impl MailboxPolicy {
  /// Creates a new policy.
  #[must_use]
  pub const fn new(
    capacity: MailboxCapacity,
    overflow: MailboxOverflowStrategy,
    throughput_limit: Option<NonZeroUsize>,
  ) -> Self {
    Self { capacity, overflow, throughput_limit }
  }

  /// Creates a bounded mailbox policy with the specified capacity and overflow strategy.
  #[must_use]
  pub fn bounded(
    capacity: NonZeroUsize,
    overflow: MailboxOverflowStrategy,
    throughput_limit: Option<NonZeroUsize>,
  ) -> Self {
    Self::new(MailboxCapacity::Bounded { capacity }, overflow, throughput_limit)
  }

  /// Creates an unbounded mailbox policy.
  #[must_use]
  pub fn unbounded(throughput_limit: Option<NonZeroUsize>) -> Self {
    Self::new(MailboxCapacity::Unbounded, MailboxOverflowStrategy::DropOldest, throughput_limit)
  }

  /// Returns the capacity configuration.
  #[must_use]
  pub const fn capacity(&self) -> MailboxCapacity {
    self.capacity
  }

  /// Returns the overflow strategy used when the mailbox is full.
  #[must_use]
  pub const fn overflow(&self) -> MailboxOverflowStrategy {
    self.overflow
  }

  /// Returns the per-turn throughput limit (if any).
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
