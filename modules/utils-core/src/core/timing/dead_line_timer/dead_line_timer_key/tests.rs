use super::DeadLineTimerKey;

#[test]
fn invalid_key_checks() {
  let invalid = DeadLineTimerKey::invalid();
  assert!(!invalid.is_valid());
  assert_eq!(invalid.as_raw(), 0);
}

#[test]
fn round_trip_raw() {
  let key = DeadLineTimerKey::from_raw(42);
  assert!(key.is_valid());
  assert_eq!(key.into_raw(), 42);
}
