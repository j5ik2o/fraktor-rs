use super::{SpinRwLockFamily, SyncRwLockFamily};
use crate::core::sync::sync_rwlock_like::SyncRwLockLike;

#[test]
fn spin_rwlock_family_creates_lock() {
  let lock = SpinRwLockFamily::create(3_u32);
  assert_eq!(*lock.read(), 3);
}
