use super::SpinSyncRwLock;
use crate::core::sync::sync_rwlock_like::SyncRwLockLike;

#[test]
fn write_updates_value() {
  let lock = SpinSyncRwLock::new(10_u32);
  {
    let mut guard = lock.write();
    *guard = 20;
  }
  assert_eq!(*lock.read(), 20);
}
