/// Overflow handling strategy for bounded mailboxes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OverflowPolicy {
  /// Drop the newest message when the capacity is exceeded.
  DropNewest,
  /// Drop the oldest message when the capacity is exceeded.
  DropOldest,
  /// Grow the mailbox by allocating additional storage.
  Grow,
  /// Block producers until capacity becomes available.
  Block,
}
