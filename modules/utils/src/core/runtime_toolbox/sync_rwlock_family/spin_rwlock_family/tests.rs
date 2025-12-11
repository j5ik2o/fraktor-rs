use super::SpinRwLockFamily;
use crate::core::{runtime_toolbox::sync_rwlock_family::SyncRwLockFamily, sync::sync_rwlock_like::SyncRwLockLike};

#[test]
fn spin_family_provides_rwlock() {
  let lock = SpinRwLockFamily::create(3_u32);
  assert_eq!(*lock.read(), 3);
}
