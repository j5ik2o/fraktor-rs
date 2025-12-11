use super::SpinSyncRwLock;

#[test]
fn read_then_write() {
  let lock = SpinSyncRwLock::new(1_u32);
  assert_eq!(*lock.read(), 1);
  {
    let mut guard = lock.write();
    *guard = 2;
  }
  assert_eq!(*lock.read(), 2);
}
