use super::{RuntimeMutex, RuntimeRwLock};
use crate::core::sync::{SpinSyncMutex, SpinSyncRwLock};

#[test]
fn runtime_mutex_protects_value() {
  let mutex: RuntimeMutex<_> = RuntimeMutex::new(5_u32);
  assert_eq!(*mutex.lock(), 5);
}

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
fn runtime_mutex_explicit_driver_matches_default() {
  let mutex: RuntimeMutex<_, SpinSyncMutex<_>> = RuntimeMutex::new(5_u32);
  assert_eq!(*mutex.lock(), 5);
}

#[test]
fn runtime_rwlock_explicit_driver_matches_default() {
  let rwlock: RuntimeRwLock<_, SpinSyncRwLock<_>> = RuntimeRwLock::new(7_u32);
  assert_eq!(*rwlock.read(), 7);
}
