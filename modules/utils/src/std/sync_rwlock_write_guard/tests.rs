use std::sync::RwLock;

use super::StdSyncRwLockWriteGuard;

#[test]
fn write_guard_updates() {
  let lock = RwLock::new(1_u32);
  {
    let guard = lock.write().unwrap();
    let mut wrapped = StdSyncRwLockWriteGuard::new(guard);
    *wrapped = 5;
  }
  assert_eq!(*lock.read().unwrap(), 5);
}
