use std::sync::RwLock;

use super::StdSyncRwLockReadGuard;

#[test]
fn read_guard_derefs() {
  let lock = RwLock::new(4_u32);
  let guard = lock.read().unwrap();
  let wrapped = StdSyncRwLockReadGuard::new(guard);
  assert_eq!(*wrapped, 4);
}
