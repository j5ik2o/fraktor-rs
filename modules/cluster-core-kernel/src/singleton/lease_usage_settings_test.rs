use core::time::Duration;

use super::LeaseUsageSettings;
use crate::singleton::ClusterSingletonSettingsError;

#[test]
fn new_holds_both_items() {
  let settings = LeaseUsageSettings::new("my-lease-impl", Duration::from_secs(2));

  assert_eq!(settings.lease_implementation(), "my-lease-impl");
  assert_eq!(settings.lease_retry_interval(), Duration::from_secs(2));
}

#[test]
fn validate_rejects_empty_lease_implementation() {
  let settings = LeaseUsageSettings::new("", Duration::from_secs(1));

  assert_eq!(settings.validate(), Err(ClusterSingletonSettingsError::EmptyLeaseImplementation));
}

#[test]
fn validate_rejects_zero_lease_retry_interval() {
  let settings = LeaseUsageSettings::new("my-lease-impl", Duration::ZERO);

  assert_eq!(settings.validate(), Err(ClusterSingletonSettingsError::NonPositiveLeaseRetryInterval));
}

#[test]
fn validate_accepts_valid_settings() {
  let settings = LeaseUsageSettings::new("my-lease-impl", Duration::from_millis(500));

  assert_eq!(settings.validate(), Ok(()));
}
