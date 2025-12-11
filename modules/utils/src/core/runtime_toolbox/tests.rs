use core::time::Duration;

use super::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, SyncRwLockFamily, ToolboxMutex, ToolboxRwLock};
use crate::core::sync::sync_rwlock_like::SyncRwLockLike;

#[test]
fn toolbox_mutex_uses_spin_family() {
  type Family = <NoStdToolbox as RuntimeToolbox>::MutexFamily;
  let mutex: ToolboxMutex<_, NoStdToolbox> = Family::create(5_u32);
  assert_eq!(*mutex.lock(), 5);
}

#[test]
fn toolbox_rwlock_uses_spin_family() {
  type Family = <NoStdToolbox as RuntimeToolbox>::RwLockFamily;
  let rwlock: ToolboxRwLock<_, NoStdToolbox> = Family::create(7_u32);
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
