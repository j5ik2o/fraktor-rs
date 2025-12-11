use super::StdSyncRwLock;
use crate::core::sync::sync_rwlock_like::SyncRwLockLike;

#[test]
fn read_write_cycle() {
  let lock = StdSyncRwLock::new(11_u32);
  assert_eq!(*lock.read(), 11);
  {
    let mut guard = lock.write();
    *guard = 13;
  }
  assert_eq!(*lock.read(), 13);
}
