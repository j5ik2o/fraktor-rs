//! Matched remote deployment response.

use crate::wire::{RemoteDeploymentCreateFailure, RemoteDeploymentCreateSuccess};

/// Response for an origin-side remote deployment request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RemoteDeploymentResponse {
  /// Successful create response.
  Success(RemoteDeploymentCreateSuccess),
  /// Failed create response.
  Failure(RemoteDeploymentCreateFailure),
}

impl RemoteDeploymentResponse {
  /// Returns the high 64 bits of the correlation id.
  #[must_use]
  pub const fn correlation_hi(&self) -> u64 {
    match self {
      | Self::Success(success) => success.correlation_hi(),
      | Self::Failure(failure) => failure.correlation_hi(),
    }
  }

  /// Returns the low 32 bits of the correlation id.
  #[must_use]
  pub const fn correlation_lo(&self) -> u32 {
    match self {
      | Self::Success(success) => success.correlation_lo(),
      | Self::Failure(failure) => failure.correlation_lo(),
    }
  }
}
