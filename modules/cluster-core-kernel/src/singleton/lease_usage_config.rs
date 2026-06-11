//! Lease usage configuration slot.

use alloc::string::String;
use core::time::Duration;

use super::ClusterSingletonConfigError;

#[cfg(test)]
#[path = "lease_usage_config_test.rs"]
mod tests;

/// Lease usage configuration: two items only (implementation name and retry interval).
///
/// `Default` is not provided because there is no meaningful default value for
/// `lease_implementation`. The absence of a lease slot is expressed by the
/// holder's `Option<LeaseUsageConfig>` field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaseUsageConfig {
  lease_implementation: String,
  lease_retry_interval: Duration,
}

impl LeaseUsageConfig {
  /// Creates a new `LeaseUsageConfig` with the given implementation name and retry interval.
  #[must_use]
  pub fn new(lease_implementation: &str, lease_retry_interval: Duration) -> Self {
    Self { lease_implementation: String::from(lease_implementation), lease_retry_interval }
  }

  /// Returns the lease implementation identifier.
  #[must_use]
  pub fn lease_implementation(&self) -> &str {
    &self.lease_implementation
  }

  /// Returns the lease retry interval.
  #[must_use]
  pub const fn lease_retry_interval(&self) -> Duration {
    self.lease_retry_interval
  }

  /// Validates this lease usage configuration.
  ///
  /// # Errors
  ///
  /// Returns [`ClusterSingletonConfigError::EmptyLeaseImplementation`] when the
  /// implementation name is empty, or
  /// [`ClusterSingletonConfigError::NonPositiveLeaseRetryInterval`] when the
  /// retry interval is zero.
  pub fn validate(&self) -> Result<(), ClusterSingletonConfigError> {
    if self.lease_implementation.is_empty() {
      return Err(ClusterSingletonConfigError::EmptyLeaseImplementation);
    }
    if self.lease_retry_interval == Duration::ZERO {
      return Err(ClusterSingletonConfigError::NonPositiveLeaseRetryInterval);
    }
    Ok(())
  }
}
