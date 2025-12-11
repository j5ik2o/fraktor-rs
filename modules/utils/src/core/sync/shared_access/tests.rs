use alloc::string::String;

use crate::core::sync::{ArcShared, SharedAccess, sync_mutex_like::SpinSyncMutex};

/// テスト用のシンプルな SharedAccess 実装（Mutex バックエンド）。
struct SharedSpin<T> {
  inner: ArcShared<SpinSyncMutex<T>>,
}

impl<T> SharedSpin<T> {
  fn new(value: T) -> Self {
    Self { inner: ArcShared::new(SpinSyncMutex::new(value)) }
  }
}

impl<T> SharedAccess<T> for SharedSpin<T> {
  fn with_read<R>(&self, f: impl FnOnce(&T) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}

#[test]
fn with_write_modifies_value() {
  let shared = SharedSpin::new(42);
  let result = shared.with_write(|value| {
    *value = 100;
    *value
  });
  assert_eq!(result, 100);
  let current_value = shared.with_read(|value: &i32| *value);
  assert_eq!(current_value, 100);
}

#[test]
fn with_write_returns_result() {
  let shared = SharedSpin::new(0);
  let result = shared.with_write(|value| {
    *value += 10;
    *value * 2
  });
  assert_eq!(result, 20);
}

#[test]
fn with_write_can_be_called_multiple_times() {
  let shared = SharedSpin::new(0);
  shared.with_write(|value| *value = 5);
  shared.with_write(|value| *value += 3);
  let final_value = shared.with_read(|value: &i32| *value);
  assert_eq!(final_value, 8);
}

#[test]
fn with_write_works_with_string() {
  let shared = SharedSpin::new(String::from("hello"));
  let result = shared.with_write(|s| {
    s.push_str(" world");
    s.len()
  });
  assert_eq!(result, 11);
  let content = shared.with_read(|value: &String| value.clone());
  assert_eq!(content, "hello world");
}
