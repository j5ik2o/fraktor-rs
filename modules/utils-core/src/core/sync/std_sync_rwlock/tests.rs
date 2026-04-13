use super::StdSyncRwLock;

#[test]
fn reads_and_writes_value() {
  let rwlock = StdSyncRwLock::new(2_u32);
  assert_eq!(*rwlock.read(), 2);
  *rwlock.write() = 4;
  assert_eq!(*rwlock.read(), 4);
}

#[test]
fn into_inner_returns_value() {
  let rwlock = StdSyncRwLock::new(vec![1, 2, 3]);
  assert_eq!(rwlock.into_inner(), vec![1, 2, 3]);
}

#[test]
fn const_new() {
  static RWLOCK: StdSyncRwLock<i32> = StdSyncRwLock::new(42);
  assert_eq!(*RWLOCK.read(), 42);
}
