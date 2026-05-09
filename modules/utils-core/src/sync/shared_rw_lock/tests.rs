use crate::sync::{SharedAccess, SharedRwLock, SpinSyncRwLock};

#[test]
fn with_read_and_with_write_access_shared_value() {
  let lock = SharedRwLock::new_with_driver::<SpinSyncRwLock<_>>(10_i32);

  assert_eq!(lock.with_read(|value| *value), 10);

  let written = lock.with_write(|value| {
    *value += 7;
    *value
  });

  assert_eq!(written, 17);
  assert_eq!(lock.with_read(|value| *value), 17);
}

#[test]
fn clone_shares_the_same_inner_value() {
  let original = SharedRwLock::new_with_driver::<SpinSyncRwLock<_>>(2_i32);
  let cloned = original.clone();

  original.with_write(|value| *value *= 3);

  assert_eq!(cloned.with_read(|value| *value), 6);
}

#[test]
fn shared_access_delegates_to_rwlock_methods() {
  let lock = SharedRwLock::new_with_driver::<SpinSyncRwLock<_>>(1_i32);

  SharedAccess::with_write(&lock, |value| *value = 9);

  assert_eq!(SharedAccess::with_read(&lock, |value| *value), 9);
}

#[test]
fn downgrade_and_upgrade_track_liveness() {
  let weak = {
    let lock = SharedRwLock::new_with_driver::<SpinSyncRwLock<_>>(4_i32);
    let weak = lock.downgrade();
    let cloned = weak.clone();

    assert_eq!(cloned.upgrade().map(|value| value.with_read(|inner| *inner)), Some(4));

    weak
  };

  assert!(weak.upgrade().is_none());
}
