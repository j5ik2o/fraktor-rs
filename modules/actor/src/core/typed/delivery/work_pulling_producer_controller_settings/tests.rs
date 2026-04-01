use core::time::Duration;

use crate::core::typed::delivery::WorkPullingProducerControllerSettings;

#[test]
fn default_settings() {
  let settings = WorkPullingProducerControllerSettings::new();
  assert_eq!(settings.buffer_size(), 1000);
}

#[test]
fn default_trait() {
  let settings = WorkPullingProducerControllerSettings::default();
  assert_eq!(settings.buffer_size(), 1000);
}

// --- T4: public builder tests ---

#[test]
fn with_buffer_size_overrides_default() {
  // Given: default settings
  let settings = WorkPullingProducerControllerSettings::new();

  // When: buffer_size is overridden
  let settings = settings.with_buffer_size(500);

  // Then: the new value is returned
  assert_eq!(settings.buffer_size(), 500);
}

#[test]
fn default_internal_ask_timeout() {
  // Given: default settings
  let settings = WorkPullingProducerControllerSettings::new();

  // Then: internal_ask_timeout has a reasonable default (5 seconds per Pekko)
  assert_eq!(settings.internal_ask_timeout(), Duration::from_secs(5));
}

#[test]
fn with_internal_ask_timeout_overrides_default() {
  // Given: default settings
  let settings = WorkPullingProducerControllerSettings::new();

  // When: internal_ask_timeout is overridden
  let settings = settings.with_internal_ask_timeout(Duration::from_secs(10));

  // Then: the new value is returned
  assert_eq!(settings.internal_ask_timeout(), Duration::from_secs(10));
}

#[test]
fn builders_preserve_other_fields() {
  // Given: settings with custom buffer_size
  let settings = WorkPullingProducerControllerSettings::new().with_buffer_size(2000);

  // When: internal_ask_timeout is overridden
  let settings = settings.with_internal_ask_timeout(Duration::from_secs(3));

  // Then: buffer_size is preserved
  assert_eq!(settings.buffer_size(), 2000);
  assert_eq!(settings.internal_ask_timeout(), Duration::from_secs(3));
}

#[test]
fn builders_chain_fluently() {
  // Given/When: full builder chain
  let settings = WorkPullingProducerControllerSettings::new()
    .with_buffer_size(750)
    .with_internal_ask_timeout(Duration::from_millis(2500));

  // Then: all values are set correctly
  assert_eq!(settings.buffer_size(), 750);
  assert_eq!(settings.internal_ask_timeout(), Duration::from_millis(2500));
}

#[test]
fn with_buffer_size_preserves_internal_ask_timeout() {
  // Given: settings with custom internal_ask_timeout
  let settings = WorkPullingProducerControllerSettings::new().with_internal_ask_timeout(Duration::from_secs(8));

  // When: buffer_size is overridden
  let settings = settings.with_buffer_size(100);

  // Then: internal_ask_timeout is preserved
  assert_eq!(settings.buffer_size(), 100);
  assert_eq!(settings.internal_ask_timeout(), Duration::from_secs(8));
}
