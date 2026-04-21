use super::SyncOnce;

#[test]
fn test_sync_once_new_is_uncompleted() {
  let once: SyncOnce<i32> = SyncOnce::new();
  assert!(!once.is_completed());
  assert!(once.get().is_none());
}

#[test]
fn test_sync_once_call_once_initializes_value() {
  let once = SyncOnce::new();
  let value = once.call_once(|| 100);
  assert_eq!(*value, 100);
  assert!(once.is_completed());
  assert_eq!(once.get(), Some(&100));
}

#[test]
fn test_sync_once_call_once_runs_only_once() {
  let once = SyncOnce::new();
  let first = *once.call_once(|| 1);
  let second = *once.call_once(|| 2);
  assert_eq!(first, 1);
  assert_eq!(second, 1);
}

#[test]
fn test_sync_once_default_matches_new() {
  let once: SyncOnce<u32> = SyncOnce::default();
  assert!(!once.is_completed());
  let value = once.call_once(|| 9);
  assert_eq!(*value, 9);
}

#[test]
fn test_sync_once_const_new() {
  static ONCE: SyncOnce<&str> = SyncOnce::new();
  let value = ONCE.call_once(|| "ready");
  assert_eq!(*value, "ready");
  assert_eq!(ONCE.get(), Some(&"ready"));
}

#[test]
fn test_sync_once_with_explicit_driver() {
  use crate::core::sync::SpinOnce;
  let once: SyncOnce<i32, SpinOnce<i32>> = SyncOnce::with_driver();
  assert!(!once.is_completed());
  let value = once.call_once(|| 21);
  assert_eq!(*value, 21);
  assert!(once.is_completed());
  assert_eq!(once.get(), Some(&21));
}

#[test]
fn test_sync_once_implements_once_driver() {
  use crate::core::sync::OnceDriver;
  let once: SyncOnce<u8> = <SyncOnce<u8> as OnceDriver<u8>>::new();
  assert!(!OnceDriver::is_completed(&once));
  assert_eq!(OnceDriver::get(&once), None);
  let value = OnceDriver::call_once(&once, || 3u8);
  assert_eq!(*value, 3);
  assert!(OnceDriver::is_completed(&once));
  assert_eq!(OnceDriver::get(&once), Some(&3));
}

#[test]
fn test_sync_once_nested_as_driver() {
  use crate::core::sync::SpinOnce;
  // SyncOnce<T> 自身が OnceDriver<T> を実装するため、SyncOnce の backend として SyncOnce を渡せる
  let outer: SyncOnce<i32, SyncOnce<i32, SpinOnce<i32>>> = SyncOnce::with_driver();
  let value = outer.call_once(|| 7);
  assert_eq!(*value, 7);
  assert!(outer.is_completed());
}
