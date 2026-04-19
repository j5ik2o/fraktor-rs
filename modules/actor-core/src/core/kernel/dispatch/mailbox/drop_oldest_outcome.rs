//! Outcome of [`QueueStateHandle::drop_oldest_and_offer`] when the queue is full.

/// Result of a drop-oldest offer operation that surfaces the evicted element so
/// callers (mailbox layer) can forward it to dead letters.
pub(crate) enum DropOldestOutcome<T> {
  /// The new element was offered without evicting an existing entry.
  Accepted,
  /// The new element was offered after evicting the oldest entry, which
  /// is returned so the caller can forward it to dead letters.
  Evicted(T),
}
