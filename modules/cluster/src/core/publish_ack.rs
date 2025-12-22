//! Publish acknowledgement payload.

use crate::core::{PublishRejectReason, PublishStatus};

/// Acknowledgement returned for publish requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishAck {
  /// Publish outcome.
  pub status: PublishStatus,
  /// Optional rejection reason.
  pub reason: Option<PublishRejectReason>,
}

impl PublishAck {
  /// Creates an accepted acknowledgement.
  #[must_use]
  pub const fn accepted() -> Self {
    Self { status: PublishStatus::Accepted, reason: None }
  }

  /// Creates a rejected acknowledgement with the provided reason.
  #[must_use]
  pub const fn rejected(reason: PublishRejectReason) -> Self {
    Self { status: PublishStatus::Rejected, reason: Some(reason) }
  }
}
