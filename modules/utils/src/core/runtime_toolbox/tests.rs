use core::time::Duration;

use super::{NoStdToolbox, RuntimeMutex, RuntimeRwLock, RuntimeToolbox};
use crate::core::sync::sync_rwlock_like::SyncRwLockLike;

#[cfg(not(feature = "std"))]
#[test]
fn runtime_lock_aliases_use_spin_backend() {
  let _: RuntimeMutex<u8> = crate::core::sync::sync_mutex_like::SpinSyncMutex::new(1);
  let _: RuntimeRwLock<u8> = crate::core::sync::sync_rwlock_like::SpinSyncRwLock::new(1);
}

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
fn tick_handle_tracks_pending_ticks() {
  let toolbox = NoStdToolbox::new(Duration::from_millis(2));
  let handle = toolbox.tick_source();
  let lease = handle.lease();
  assert!(lease.try_pull().is_none());

  handle.inject_manual_ticks(3);
  let event = lease.try_pull().expect("ticks available");
  assert_eq!(event.ticks(), 3);
  assert!(lease.try_pull().is_none());
}
