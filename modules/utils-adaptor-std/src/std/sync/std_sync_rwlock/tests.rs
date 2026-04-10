use super::StdSyncRwLock;

#[test]
fn reads_and_writes_value() {
  let rwlock = StdSyncRwLock::new(2_u32);
  assert_eq!(*rwlock.read(), 2);
  *rwlock.write() = 4;
  assert_eq!(*rwlock.read(), 4);
}
