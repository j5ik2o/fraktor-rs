use crate::core::ThrottleMode;

#[test]
fn shaping_and_enforcing_are_distinct_variants() {
  assert_ne!(ThrottleMode::Shaping, ThrottleMode::Enforcing);
}

#[test]
fn throttle_mode_is_clone_and_copy() {
  let mode = ThrottleMode::Shaping;
  let cloned = mode;
  assert_eq!(mode, cloned);
}

#[test]
fn throttle_mode_debug_output_contains_variant_name() {
  let shaping = alloc::format!("{:?}", ThrottleMode::Shaping);
  let enforcing = alloc::format!("{:?}", ThrottleMode::Enforcing);
  assert!(shaping.contains("Shaping"));
  assert!(enforcing.contains("Enforcing"));
}
