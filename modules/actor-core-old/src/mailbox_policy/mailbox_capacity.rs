use core::num::NonZeroUsize;

/// Capacity strategy for the mailbox.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MailboxCapacity {
  /// Capacity is fixed and enforced.
  Bounded {
    /// Maximum number of messages stored before overflow behaviour engages.
    capacity: NonZeroUsize,
  },
  /// Capacity is unbounded and may consume additional heap memory.
  Unbounded,
}
