use core::time::Duration;

use super::LeaseUsageConfig;
use crate::singleton::ClusterSingletonConfigError;

#[test]
fn new_holds_both_items() {
  let config = LeaseUsageConfig::new("my-lease-impl", Duration::from_secs(2));

  assert_eq!(config.lease_implementation(), "my-lease-impl");
  assert_eq!(config.lease_retry_interval(), Duration::from_secs(2));
}

#[test]
fn validate_rejects_empty_lease_implementation() {
  let config = LeaseUsageConfig::new("", Duration::from_secs(1));

  assert_eq!(config.validate(), Err(ClusterSingletonConfigError::EmptyLeaseImplementation));
}

#[test]
fn validate_rejects_zero_lease_retry_interval() {
  let config = LeaseUsageConfig::new("my-lease-impl", Duration::ZERO);

  assert_eq!(config.validate(), Err(ClusterSingletonConfigError::NonPositiveLeaseRetryInterval));
}

#[test]
fn validate_accepts_valid_config() {
  let config = LeaseUsageConfig::new("my-lease-impl", Duration::from_millis(500));

  assert_eq!(config.validate(), Ok(()));
}
