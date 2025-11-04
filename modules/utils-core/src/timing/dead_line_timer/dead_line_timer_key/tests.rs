use super::DeadLineTimerKey;

#[test]
fn deadline_timer_key_invalid() {
  let key = DeadLineTimerKey::invalid();
  assert!(!key.is_valid());
  assert_eq!(key.into_raw(), 0);
}

#[test]
fn deadline_timer_key_is_valid() {
  let key = DeadLineTimerKey::from_raw(1);
  assert!(key.is_valid());

  let key2 = DeadLineTimerKey::from_raw(100);
  assert!(key2.is_valid());

  let invalid = DeadLineTimerKey::invalid();
  assert!(!invalid.is_valid());
}

#[test]
fn deadline_timer_key_into_raw() {
  let key = DeadLineTimerKey::from_raw(42);
  assert_eq!(key.into_raw(), 42);

  let key2 = DeadLineTimerKey::from_raw(0);
  assert_eq!(key2.into_raw(), 0);
}

#[test]
fn deadline_timer_key_from_raw() {
  let key = DeadLineTimerKey::from_raw(123);
  assert_eq!(key.into_raw(), 123);
  assert!(key.is_valid());
}

#[test]
fn deadline_timer_key_default() {
  let key = DeadLineTimerKey::default();
  assert_eq!(key.into_raw(), 0);
  assert!(!key.is_valid());
}

#[test]
fn deadline_timer_key_clone() {
  let key1 = DeadLineTimerKey::from_raw(50);
  let key2 = key1; // Copy trait???clone()??
  assert_eq!(key1, key2);
}

#[test]
fn deadline_timer_key_copy() {
  let key1 = DeadLineTimerKey::from_raw(99);
  let key2 = key1;
  assert_eq!(key1, key2);
  assert_eq!(key1.into_raw(), 99);
  assert_eq!(key2.into_raw(), 99);
}

#[test]
fn deadline_timer_key_debug() {
  let key = DeadLineTimerKey::from_raw(200);
  let debug_str = format!("{:?}", key);
  assert!(debug_str.contains("DeadlineTimerKey"));
}

#[test]
fn deadline_timer_key_partial_eq() {
  let key1 = DeadLineTimerKey::from_raw(10);
  let key2 = DeadLineTimerKey::from_raw(10);
  let key3 = DeadLineTimerKey::from_raw(20);
  assert_eq!(key1, key2);
  assert_ne!(key1, key3);
}

#[test]
fn deadline_timer_key_eq() {
  let key1 = DeadLineTimerKey::from_raw(30);
  let key2 = DeadLineTimerKey::from_raw(30);
  assert_eq!(key1, key2);
}

#[test]
fn deadline_timer_key_partial_ord() {
  let key1 = DeadLineTimerKey::from_raw(10);
  let key2 = DeadLineTimerKey::from_raw(20);
  assert!(key1 < key2);
  assert!(key2 > key1);
  assert!(key1 <= key2);
  assert!(key2 >= key1);
}

#[test]
fn deadline_timer_key_ord() {
  let mut keys = vec![DeadLineTimerKey::from_raw(30), DeadLineTimerKey::from_raw(10), DeadLineTimerKey::from_raw(20)];
  keys.sort();
  assert_eq!(keys[0].into_raw(), 10);
  assert_eq!(keys[1].into_raw(), 20);
  assert_eq!(keys[2].into_raw(), 30);
}
