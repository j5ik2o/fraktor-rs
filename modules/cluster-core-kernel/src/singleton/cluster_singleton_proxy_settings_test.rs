use core::time::Duration;

use super::ClusterSingletonProxySettings;
use crate::{membership::DataCenter, singleton::ClusterSingletonSettingsError};

// --- 既定値テスト（要件 2.1, 2.2） ---

#[test]
fn default_settings_have_pekko_compatible_defaults() {
  let s = ClusterSingletonProxySettings::new();

  assert_eq!(s.singleton_name(), "singleton");
  assert_eq!(s.role(), None);
  assert_eq!(s.data_center(), None);
  assert_eq!(s.singleton_identification_interval(), Duration::from_secs(1));
  assert_eq!(s.buffer_size(), 1000);
}

#[test]
fn default_trait_delegates_to_new() {
  let a = ClusterSingletonProxySettings::new();
  let b = ClusterSingletonProxySettings::default();

  assert_eq!(a, b);
}

// --- builder テスト ---

#[test]
fn builder_setters_update_fields() {
  let dc = DataCenter::new("us-east-1");
  let s = ClusterSingletonProxySettings::new()
    .with_singleton_name("my-singleton")
    .with_role("my-role")
    .with_data_center(dc.clone())
    .with_singleton_identification_interval(Duration::from_millis(500))
    .with_buffer_size(500);

  assert_eq!(s.singleton_name(), "my-singleton");
  assert_eq!(s.role(), Some("my-role"));
  assert_eq!(s.data_center(), Some(&dc));
  assert_eq!(s.singleton_identification_interval(), Duration::from_millis(500));
  assert_eq!(s.buffer_size(), 500);
}

// --- 検証テスト（要件 4.2, 4.3, 4.4） ---

#[test]
fn validate_rejects_empty_singleton_name() {
  let result = ClusterSingletonProxySettings::new().with_singleton_name("").validate();

  assert_eq!(result, Err(ClusterSingletonSettingsError::EmptySingletonName));
}

#[test]
fn validate_rejects_zero_identification_interval() {
  let result = ClusterSingletonProxySettings::new().with_singleton_identification_interval(Duration::ZERO).validate();

  assert_eq!(result, Err(ClusterSingletonSettingsError::NonPositiveIdentificationInterval));
}

#[test]
fn validate_rejects_buffer_size_over_10000() {
  let result = ClusterSingletonProxySettings::new().with_buffer_size(10001).validate();

  assert_eq!(result, Err(ClusterSingletonSettingsError::BufferSizeOutOfRange { value: 10001 }));
}

// --- buffer size 0 受理テスト（要件 2.3） ---

#[test]
fn validate_accepts_buffer_size_zero_as_no_buffering() {
  let result = ClusterSingletonProxySettings::new().with_buffer_size(0).validate();

  assert_eq!(result, Ok(()));
}

#[test]
fn validate_accepts_buffer_size_10000_as_upper_bound() {
  let result = ClusterSingletonProxySettings::new().with_buffer_size(10000).validate();

  assert_eq!(result, Ok(()));
}

#[test]
fn validate_passes_for_default_settings() {
  assert_eq!(ClusterSingletonProxySettings::new().validate(), Ok(()));
}

// --- difference_field_names テスト（要件 5.2） ---

#[test]
fn difference_field_names_returns_empty_when_equal() {
  let s = ClusterSingletonProxySettings::new();

  assert!(s.difference_field_names(&s).is_empty());
}

#[test]
fn difference_field_names_returns_single_changed_field() {
  let base = ClusterSingletonProxySettings::new();
  let changed = ClusterSingletonProxySettings::new().with_singleton_name("other");

  assert_eq!(changed.difference_field_names(&base).as_slice(), ["singleton_name"]);
}

#[test]
fn difference_field_names_returns_all_changed_fields() {
  let dc = DataCenter::new("eu-west-1");
  let base = ClusterSingletonProxySettings::new();
  let changed = ClusterSingletonProxySettings::new()
    .with_singleton_name("other")
    .with_role("r")
    .with_data_center(dc)
    .with_singleton_identification_interval(Duration::from_millis(500))
    .with_buffer_size(500);

  let names = changed.difference_field_names(&base);
  assert_eq!(names.as_slice(), [
    "singleton_name",
    "role",
    "data_center",
    "singleton_identification_interval",
    "buffer_size",
  ]);
}
