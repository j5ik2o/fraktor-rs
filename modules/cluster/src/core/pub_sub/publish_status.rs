//! Publish acceptance status.

/// Indicates whether a publish was accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublishStatus {
  /// Publish accepted.
  Accepted,
  /// Publish rejected.
  Rejected,
}
