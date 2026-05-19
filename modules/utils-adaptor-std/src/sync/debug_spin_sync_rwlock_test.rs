use super::DebugSpinSyncRwLock;

#[test]
fn reads_and_writes_value() {
  let rwlock = DebugSpinSyncRwLock::new(5_u32);
  assert_eq!(*rwlock.read(), 5);
  *rwlock.write() = 8;
  assert_eq!(*rwlock.read(), 8);
}
