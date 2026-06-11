//! Lease usage settings slot.

use alloc::string::String;
use core::time::Duration;

use super::ClusterSingletonSettingsError;

#[cfg(test)]
#[path = "lease_usage_settings_test.rs"]
mod tests;

/// Lease usage settings: two items only (implementation name and retry interval).
///
/// `Default` is not provided because there is no meaningful default value for
/// `lease_implementation`. The absence of a lease slot is expressed by the
/// holder's `Option<LeaseUsageSettings>` field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaseUsageSettings {
  lease_implementation: String,
  lease_retry_interval: Duration,
}

impl LeaseUsageSettings {
  /// Creates a new `LeaseUsageSettings` with the given implementation name and retry interval.
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

  /// Validates this lease usage settings.
  ///
  /// # Errors
  ///
  /// Returns [`ClusterSingletonSettingsError::EmptyLeaseImplementation`] when the
  /// implementation name is empty, or
  /// [`ClusterSingletonSettingsError::NonPositiveLeaseRetryInterval`] when the
  /// retry interval is zero.
  pub fn validate(&self) -> Result<(), ClusterSingletonSettingsError> {
    if self.lease_implementation.is_empty() {
      return Err(ClusterSingletonSettingsError::EmptyLeaseImplementation);
    }
    if self.lease_retry_interval == Duration::ZERO {
      return Err(ClusterSingletonSettingsError::NonPositiveLeaseRetryInterval);
    }
    Ok(())
  }
}
