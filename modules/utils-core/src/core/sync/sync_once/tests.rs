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
