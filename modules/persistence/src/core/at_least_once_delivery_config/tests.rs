use core::time::Duration;

use crate::core::at_least_once_delivery_config::AtLeastOnceDeliveryConfig;

#[test]
fn config_accessors_return_values() {
  let config = AtLeastOnceDeliveryConfig::new(Duration::from_secs(5), 7, 3, 9);
  assert_eq!(config.redeliver_interval(), Duration::from_secs(5));
  assert_eq!(config.max_unconfirmed(), 7);
  assert_eq!(config.redelivery_burst_limit(), 3);
  assert_eq!(config.warn_after_number_of_unconfirmed_attempts(), 9);
}

#[test]
fn config_default_is_reasonable() {
  let config = AtLeastOnceDeliveryConfig::default();
  assert!(config.max_unconfirmed() > 0);
  assert!(config.redelivery_burst_limit() > 0);
  assert!(config.warn_after_number_of_unconfirmed_attempts() > 0);
}
