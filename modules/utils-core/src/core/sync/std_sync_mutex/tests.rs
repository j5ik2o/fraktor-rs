use super::StdSyncMutex;

#[test]
fn locks_and_mutates_value() {
  let mutex = StdSyncMutex::new(1_u32);
  *mutex.lock() = 3;
  assert_eq!(*mutex.lock(), 3);
}

#[test]
fn into_inner_returns_value() {
  let mutex = StdSyncMutex::new(vec![1, 2, 3]);
  assert_eq!(mutex.into_inner(), vec![1, 2, 3]);
}

#[test]
fn const_new() {
  static MUTEX: StdSyncMutex<i32> = StdSyncMutex::new(42);
  assert_eq!(*MUTEX.lock(), 42);
}
