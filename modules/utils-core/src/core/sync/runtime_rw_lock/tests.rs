use super::RuntimeRwLock;
use crate::core::sync::SpinSyncRwLock;

#[test]
fn runtime_rwlock_protects_value() {
  let rwlock: RuntimeRwLock<_> = RuntimeRwLock::new(7_u32);
  assert_eq!(*rwlock.read(), 7);
  {
    let mut guard = rwlock.write();
    *guard = 9;
  }
  assert_eq!(*rwlock.read(), 9);
}

#[test]
fn runtime_rwlock_explicit_driver_matches_default() {
  let rwlock: RuntimeRwLock<_, SpinSyncRwLock<_>> = RuntimeRwLock::new_with_driver(7_u32);
  assert_eq!(*rwlock.read(), 7);
}
