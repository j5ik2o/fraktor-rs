//! Remote authority error types.

/// Error type for remote authority operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteAuthorityError {
  /// Authority is quarantined and cannot accept messages.
  Quarantined,
}
