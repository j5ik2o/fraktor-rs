use core::time::Duration;

use super::ConsumerControllerSettings;

#[test]
fn default_settings() {
  let settings = ConsumerControllerSettings::new();
  assert_eq!(settings.flow_control_window(), 50);
  assert!(!settings.only_flow_control());
}

#[test]
fn with_flow_control_window() {
  let settings = ConsumerControllerSettings::new().with_flow_control_window(100);
  assert_eq!(settings.flow_control_window(), 100);
}

#[test]
fn with_only_flow_control() {
  let settings = ConsumerControllerSettings::new().with_only_flow_control(true);
  assert!(settings.only_flow_control());
}

// --- T3: resend interval tests ---

#[test]
fn default_resend_interval_min() {
  // Given: default settings
  let settings = ConsumerControllerSettings::new();

  // Then: resend_interval_min has a reasonable default (2 seconds per Pekko)
  assert_eq!(settings.resend_interval_min(), Duration::from_secs(2));
}

#[test]
fn default_resend_interval_max() {
  // Given: default settings
  let settings = ConsumerControllerSettings::new();

  // Then: resend_interval_max has a reasonable default (30 seconds per Pekko)
  assert_eq!(settings.resend_interval_max(), Duration::from_secs(30));
}

#[test]
fn with_resend_interval_min_overrides_default() {
  // Given: default settings
  let settings = ConsumerControllerSettings::new();

  // When: resend_interval_min is overridden
  let settings = settings.with_resend_interval_min(Duration::from_millis(500));

  // Then: the new value is returned
  assert_eq!(settings.resend_interval_min(), Duration::from_millis(500));
}

#[test]
fn with_resend_interval_max_overrides_default() {
  // Given: default settings
  let settings = ConsumerControllerSettings::new();

  // When: resend_interval_max is overridden
  let settings = settings.with_resend_interval_max(Duration::from_secs(60));

  // Then: the new value is returned
  assert_eq!(settings.resend_interval_max(), Duration::from_secs(60));
}

#[test]
fn resend_interval_builders_preserve_other_fields() {
  // Given: settings with custom flow_control_window and only_flow_control
  let settings = ConsumerControllerSettings::new().with_flow_control_window(200).with_only_flow_control(true);

  // When: resend interval builders are applied
  let settings =
    settings.with_resend_interval_min(Duration::from_millis(100)).with_resend_interval_max(Duration::from_secs(10));

  // Then: existing fields are preserved
  assert_eq!(settings.flow_control_window(), 200);
  assert!(settings.only_flow_control());
  assert_eq!(settings.resend_interval_min(), Duration::from_millis(100));
  assert_eq!(settings.resend_interval_max(), Duration::from_secs(10));
}

#[test]
fn resend_interval_builders_chain_fluently() {
  // Given/When: full builder chain
  let settings = ConsumerControllerSettings::new()
    .with_flow_control_window(75)
    .with_resend_interval_min(Duration::from_secs(1))
    .with_resend_interval_max(Duration::from_secs(15))
    .with_only_flow_control(false);

  // Then: all values are set correctly
  assert_eq!(settings.flow_control_window(), 75);
  assert_eq!(settings.resend_interval_min(), Duration::from_secs(1));
  assert_eq!(settings.resend_interval_max(), Duration::from_secs(15));
  assert!(!settings.only_flow_control());
}
