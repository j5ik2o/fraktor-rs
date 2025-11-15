use super::*;

#[test]
fn priority_backend_config_new() {
  let config = PriorityBackendConfig::new(10, 0, 5, 2);
  assert_eq!(config.capacity(), 10);
  assert_eq!(config.min_priority(), 0);
  assert_eq!(config.max_priority(), 5);
  assert_eq!(config.default_priority(), 2);
}

#[test]
fn priority_backend_config_with_default_layout() {
  let config = PriorityBackendConfig::with_default_layout(20);
  assert_eq!(config.capacity(), 20);
  assert_eq!(config.min_priority(), 0);
  assert_eq!(config.max_priority(), (PRIORITY_LEVELS - 1) as i8);
  assert_eq!(config.default_priority(), DEFAULT_PRIORITY);
}

#[test]
fn priority_backend_config_clamp_priority_within_range() {
  let config = PriorityBackendConfig::new(10, 0, 5, 2);
  assert_eq!(config.clamp_priority(3), 3);
}

#[test]
fn priority_backend_config_clamp_priority_below_min() {
  let config = PriorityBackendConfig::new(10, 0, 5, 2);
  assert_eq!(config.clamp_priority(-1), 0);
}

#[test]
fn priority_backend_config_clamp_priority_above_max() {
  let config = PriorityBackendConfig::new(10, 0, 5, 2);
  assert_eq!(config.clamp_priority(10), 5);
}

#[test]
#[should_panic(expected = "min_priority must not exceed max_priority")]
fn priority_backend_config_new_panics_on_invalid_range() {
  let _ = PriorityBackendConfig::new(10, 5, 0, 2);
}

#[test]
#[should_panic(expected = "default_priority must be within bounds")]
fn priority_backend_config_new_panics_on_default_below_min() {
  let _ = PriorityBackendConfig::new(10, 0, 5, -1);
}

#[test]
#[should_panic(expected = "default_priority must be within bounds")]
fn priority_backend_config_new_panics_on_default_above_max() {
  let _ = PriorityBackendConfig::new(10, 0, 5, 10);
}

#[test]
fn priority_backend_config_accessors() {
  let config = PriorityBackendConfig::new(100, -10, 10, 0);
  assert_eq!(config.capacity(), 100);
  assert_eq!(config.min_priority(), -10);
  assert_eq!(config.max_priority(), 10);
  assert_eq!(config.default_priority(), 0);
}
