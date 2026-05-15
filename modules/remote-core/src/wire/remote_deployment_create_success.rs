//! Remote deployment create success PDU payload.

use alloc::string::String;

/// Response indicating that a remote deployment create request succeeded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteDeploymentCreateSuccess {
  correlation_hi: u64,
  correlation_lo: u32,
  actor_path:     String,
}

impl RemoteDeploymentCreateSuccess {
  /// Creates a remote deployment create success response.
  #[must_use]
  pub const fn new(correlation_hi: u64, correlation_lo: u32, actor_path: String) -> Self {
    Self { correlation_hi, correlation_lo, actor_path }
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

  /// Returns the canonical remote actor path created on the target node.
  #[must_use]
  pub fn actor_path(&self) -> &str {
    &self.actor_path
  }
}
