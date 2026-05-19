use crate::sync::{SharedAccess, SharedLock, SpinSyncMutex};

#[test]
fn with_lock_updates_value() {
  let lock = SharedLock::new_with_driver::<SpinSyncMutex<_>>(1_i32);

  let result = lock.with_lock(|value| {
    *value += 1;
    *value
  });

  assert_eq!(result, 2);
  assert_eq!(SharedAccess::with_read(&lock, |value| *value), 2);
}

#[test]
fn shared_access_delegates_to_with_lock() {
  let lock = SharedLock::new_with_driver::<SpinSyncMutex<_>>(3_i32);

  let written = SharedAccess::with_write(&lock, |value| {
    *value *= 4;
    *value
  });

  assert_eq!(written, 12);
  assert_eq!(SharedAccess::with_read(&lock, |value| *value), 12);
}

#[test]
fn downgrade_and_upgrade_track_liveness() {
  let weak = {
    let lock = SharedLock::new_with_driver::<SpinSyncMutex<_>>(5_i32);
    let weak = lock.downgrade();
    let cloned = weak.clone();

    assert_eq!(cloned.upgrade().map(|value| value.with_lock(|inner| *inner)), Some(5));

    weak
  };

  assert!(weak.upgrade().is_none());
}
