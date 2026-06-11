use core::time::Duration;

use super::ClusterSingletonManagerSettings;
use crate::singleton::{ClusterSingletonSettingsError, LeaseUsageSettings};

// --- 既定値テスト ---

#[test]
fn default_settings_have_pekko_compatible_defaults() {
  let s = ClusterSingletonManagerSettings::new();

  assert_eq!(s.singleton_name(), "singleton");
  assert_eq!(s.role(), None);
  assert_eq!(s.removal_margin(), None);
  assert_eq!(s.hand_over_retry_interval(), Duration::from_secs(1));
  assert_eq!(s.min_hand_over_retries(), 15);
  assert_eq!(s.lease_settings(), None);
}

#[test]
fn default_trait_delegates_to_new() {
  let a = ClusterSingletonManagerSettings::new();
  let b = ClusterSingletonManagerSettings::default();

  assert_eq!(a, b);
}

// --- Option 区別テスト（要件 1.3） ---

#[test]
fn removal_margin_none_is_distinct_from_explicit_zero() {
  let no_margin = ClusterSingletonManagerSettings::new();
  let explicit_zero = ClusterSingletonManagerSettings::new().with_removal_margin(Duration::ZERO);

  assert_eq!(no_margin.removal_margin(), None);
  assert_eq!(explicit_zero.removal_margin(), Some(Duration::ZERO));
}

// --- builder テスト ---

#[test]
fn builder_setters_update_fields() {
  let lease = LeaseUsageSettings::new("my-lease", Duration::from_secs(2));
  let s = ClusterSingletonManagerSettings::new()
    .with_singleton_name("my-singleton")
    .with_role("my-role")
    .with_removal_margin(Duration::from_secs(10))
    .with_hand_over_retry_interval(Duration::from_millis(500))
    .with_min_hand_over_retries(20)
    .with_lease_settings(lease.clone());

  assert_eq!(s.singleton_name(), "my-singleton");
  assert_eq!(s.role(), Some("my-role"));
  assert_eq!(s.removal_margin(), Some(Duration::from_secs(10)));
  assert_eq!(s.hand_over_retry_interval(), Duration::from_millis(500));
  assert_eq!(s.min_hand_over_retries(), 20);
  assert_eq!(s.lease_settings(), Some(&lease));
}

// --- 検証テスト（要件 4.3, 4.4） ---

#[test]
fn validate_rejects_empty_singleton_name() {
  let result = ClusterSingletonManagerSettings::new().with_singleton_name("").validate();

  assert_eq!(result, Err(ClusterSingletonSettingsError::EmptySingletonName));
}

#[test]
fn validate_rejects_zero_hand_over_retry_interval() {
  let result = ClusterSingletonManagerSettings::new().with_hand_over_retry_interval(Duration::ZERO).validate();

  assert_eq!(result, Err(ClusterSingletonSettingsError::NonPositiveHandOverRetryInterval));
}

#[test]
fn validate_delegates_to_lease_settings_when_present() {
  let bad_lease = LeaseUsageSettings::new("", Duration::from_secs(1));
  let result = ClusterSingletonManagerSettings::new().with_lease_settings(bad_lease).validate();

  assert_eq!(result, Err(ClusterSingletonSettingsError::EmptyLeaseImplementation));
}

#[test]
fn validate_passes_for_default_settings() {
  assert_eq!(ClusterSingletonManagerSettings::new().validate(), Ok(()));
}

// --- max_hand_over_retries 導出テスト（要件 7.1） ---

#[test]
fn max_hand_over_retries_with_no_margin_returns_min_retries() {
  // margin なし → max(15, 0 + 3) = max(15, 3) = 15
  let s = ClusterSingletonManagerSettings::new();

  assert_eq!(s.max_hand_over_retries(), 15);
}

#[test]
fn max_hand_over_retries_with_margin_26s_interval_1s_returns_29() {
  // margin 26s / 間隔 1s → margin_ticks = 26, max(15, 26 + 3) = max(15, 29) = 29
  let s = ClusterSingletonManagerSettings::new()
    .with_removal_margin(Duration::from_secs(26))
    .with_hand_over_retry_interval(Duration::from_secs(1));

  assert_eq!(s.max_hand_over_retries(), 29);
}

#[test]
fn max_hand_over_retries_does_not_panic_on_zero_interval() {
  // ゼロ間隔でも panic しない（margin_ticks = 0 として扱う）
  let s = ClusterSingletonManagerSettings::new()
    .with_removal_margin(Duration::from_secs(10))
    .with_hand_over_retry_interval(Duration::ZERO);

  // ゼロ間隔: margin_ticks = 0, max(15, 0 + 3) = 15
  assert_eq!(s.max_hand_over_retries(), 15);
}

#[test]
fn max_hand_over_retries_is_deterministic() {
  let s = ClusterSingletonManagerSettings::new()
    .with_removal_margin(Duration::from_secs(26))
    .with_hand_over_retry_interval(Duration::from_secs(1));

  assert_eq!(s.max_hand_over_retries(), s.max_hand_over_retries());
}

#[test]
fn max_hand_over_retries_uses_min_when_margin_is_small() {
  // margin 3s / 間隔 2s → margin_ticks = 1, max(15, 1 + 3) = max(15, 4) = 15
  let s = ClusterSingletonManagerSettings::new()
    .with_removal_margin(Duration::from_secs(3))
    .with_hand_over_retry_interval(Duration::from_secs(2));

  assert_eq!(s.max_hand_over_retries(), 15);
}

// --- difference_field_names テスト（要件 5.2） ---

#[test]
fn difference_field_names_returns_empty_when_equal() {
  let s = ClusterSingletonManagerSettings::new();

  assert!(s.difference_field_names(&s).is_empty());
}

#[test]
fn difference_field_names_returns_single_changed_field() {
  let base = ClusterSingletonManagerSettings::new();
  let changed = ClusterSingletonManagerSettings::new().with_singleton_name("other");

  assert_eq!(changed.difference_field_names(&base).as_slice(), ["singleton_name"]);
}

#[test]
fn difference_field_names_returns_all_changed_fields() {
  let lease = LeaseUsageSettings::new("impl", Duration::from_secs(1));
  let base = ClusterSingletonManagerSettings::new();
  let changed = ClusterSingletonManagerSettings::new()
    .with_singleton_name("other")
    .with_role("r")
    .with_removal_margin(Duration::from_secs(5))
    .with_hand_over_retry_interval(Duration::from_millis(500))
    .with_min_hand_over_retries(20)
    .with_lease_settings(lease);

  let names = changed.difference_field_names(&base);
  assert_eq!(names.as_slice(), [
    "singleton_name",
    "role",
    "removal_margin",
    "hand_over_retry_interval",
    "min_hand_over_retries",
    "lease_settings",
  ]);
}
