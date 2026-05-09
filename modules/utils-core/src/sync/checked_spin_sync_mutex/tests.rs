use crate::sync::{CheckedSpinSyncMutex, LockDriver, SharedAccess, SharedLock};

#[test]
fn normal_lock_unlock_cycle() {
  let mutex = CheckedSpinSyncMutex::new(42_u32);
  {
    let mut guard = mutex.lock();
    assert_eq!(*guard, 42);
    *guard = 99;
  }
  let guard = mutex.lock();
  assert_eq!(*guard, 99);
}

#[test]
#[should_panic(expected = "re-entrant lock")]
fn reentrant_lock_panics() {
  let mutex = CheckedSpinSyncMutex::new(0_u32);
  let _guard = mutex.lock();
  let _guard2 = mutex.lock(); // re-entry → panic
}

#[test]
fn into_inner_returns_value() {
  let mutex = CheckedSpinSyncMutex::new(7_u32);
  assert_eq!(mutex.into_inner(), 7);
}

#[test]
fn shared_lock_construction() {
  let shared = SharedLock::new_with_driver::<CheckedSpinSyncMutex<_>>(100_u32);
  shared.with_read(|v| assert_eq!(*v, 100));
  shared.with_lock(|v| *v = 200);
  shared.with_read(|v| assert_eq!(*v, 200));
}

#[test]
fn lock_driver_trait_impl() {
  let mutex = <CheckedSpinSyncMutex<u32> as LockDriver<u32>>::new(5);
  {
    let guard = LockDriver::lock(&mutex);
    assert_eq!(*guard, 5);
  }
  assert_eq!(LockDriver::into_inner(mutex), 5);
}
