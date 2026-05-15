//! Remote deployment create failure PDU payload.

use alloc::string::String;

use crate::wire::RemoteDeploymentFailureCode;

/// Response indicating that a remote deployment create request failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteDeploymentCreateFailure {
  correlation_hi: u64,
  correlation_lo: u32,
  code:           RemoteDeploymentFailureCode,
  reason:         String,
}

impl RemoteDeploymentCreateFailure {
  /// Creates a remote deployment create failure response.
  #[must_use]
  pub const fn new(
    correlation_hi: u64,
    correlation_lo: u32,
    code: RemoteDeploymentFailureCode,
    reason: String,
  ) -> Self {
    Self { correlation_hi, correlation_lo, code, reason }
  }

  /// Returns the high 64 bits of the correlation id.
  #[must_use]
  pub const fn correlation_hi(&self) -> u64 {
    self.correlation_hi
  }

  /// Returns the low 32 bits of the correlation id.
  #[must_use]
  pub const fn correlation_lo(&self) -> u32 {
    self.correlation_lo
  }

  /// Returns the structured failure code.
  #[must_use]
  pub const fn code(&self) -> RemoteDeploymentFailureCode {
    self.code
  }

  /// Returns the human-readable failure reason.
  #[must_use]
  pub fn reason(&self) -> &str {
    &self.reason
  }
}
