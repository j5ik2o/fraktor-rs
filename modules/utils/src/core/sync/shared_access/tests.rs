use alloc::string::String;

use crate::core::sync::{ArcShared, SharedAccess, sync_mutex_like::SpinSyncMutex};

#[test]
fn with_mut_modifies_value() {
  let shared: ArcShared<SpinSyncMutex<i32>> = ArcShared::new(SpinSyncMutex::new(42));
  let result = shared.with_write(|value| {
    *value = 100;
    *value
  });
  assert_eq!(result, 100);
  let current_value = shared.with_read(|value: &i32| *value);
  assert_eq!(current_value, 100);
}

#[test]
fn with_mut_returns_result() {
  let shared: ArcShared<SpinSyncMutex<i32>> = ArcShared::new(SpinSyncMutex::new(0));
  let result = shared.with_write(|value| {
    *value += 10;
    *value * 2
  });
  assert_eq!(result, 20);
}

#[test]
fn with_mut_can_be_called_multiple_times() {
  let shared: ArcShared<SpinSyncMutex<i32>> = ArcShared::new(SpinSyncMutex::new(0));
  shared.with_write(|value| *value = 5);
  shared.with_write(|value| *value += 3);
  let final_value = shared.with_read(|value: &i32| *value);
  assert_eq!(final_value, 8);
}

#[test]
fn with_mut_works_with_string() {
  let shared: ArcShared<SpinSyncMutex<String>> = ArcShared::new(SpinSyncMutex::new(String::from("hello")));
  let result = shared.with_write(|s| {
    s.push_str(" world");
    s.len()
  });
  assert_eq!(result, 11);
  let content = shared.with_read(|value: &String| value.clone());
  assert_eq!(content, "hello world");
}
