//! Capacity strategy applied to actor mailboxes.

#[cfg(test)]
mod tests;

use core::num::NonZeroUsize;

/// Configures how many messages a mailbox may hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MailboxCapacity {
  /// Capacity is fixed and enforced.
  Bounded {
    /// Maximum number of messages the mailbox stores before overflow handling kicks in.
    capacity: NonZeroUsize,
  },
  /// Capacity is unbounded and may grow until memory pressure applies.
  Unbounded,
}
