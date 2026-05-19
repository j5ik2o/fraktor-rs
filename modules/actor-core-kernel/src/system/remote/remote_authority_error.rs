//! Remote authority error types.

/// Error type for remote authority operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteAuthorityError {
  /// Authority is quarantined and cannot accept messages.
  Quarantined,
  /// Deferred message queue is full and cannot accept more messages.
  DeferredQueueFull,
}
