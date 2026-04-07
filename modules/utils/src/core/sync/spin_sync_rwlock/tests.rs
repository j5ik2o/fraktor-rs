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

#[test]
fn write_updates_value() {
  let lock = SpinSyncRwLock::new(10_u32);
  {
    let mut guard = lock.write();
    *guard = 20;
  }
  assert_eq!(*lock.read(), 20);
}
