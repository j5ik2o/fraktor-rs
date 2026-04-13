use crate::core::sync::{CheckedSpinSyncRwLock, RwLockDriver, SharedRwLock};

#[test]
fn normal_read_write_cycle() {
  let lock = CheckedSpinSyncRwLock::new(42_u32);
  {
    let guard = lock.read();
    assert_eq!(*guard, 42);
  }
  {
    let mut guard = lock.write();
    *guard = 99;
  }
  let guard = lock.read();
  assert_eq!(*guard, 99);
}

#[test]
#[should_panic(expected = "re-entrant write lock")]
fn write_reentry_panics() {
  let lock = CheckedSpinSyncRwLock::new(0_u32);
  let _guard = lock.write();
  let _guard2 = lock.write(); // re-entry → panic
}

#[test]
#[should_panic(expected = "write lock while read lock held")]
fn read_then_write_panics() {
  let lock = CheckedSpinSyncRwLock::new(0_u32);
  let _guard = lock.read();
  let _guard2 = lock.write(); // read→write upgrade → panic
}

#[test]
#[should_panic(expected = "read lock while write lock held")]
fn write_then_read_panics() {
  let lock = CheckedSpinSyncRwLock::new(0_u32);
  let _guard = lock.write();
  let _guard2 = lock.read(); // write→read → panic
}

#[test]
fn into_inner_returns_value() {
  let lock = CheckedSpinSyncRwLock::new(7_u32);
  assert_eq!(lock.into_inner(), 7);
}

#[test]
fn shared_rw_lock_construction() {
  let shared = SharedRwLock::new_with_driver::<CheckedSpinSyncRwLock<_>>(100_u32);
  shared.with_read(|v| assert_eq!(*v, 100));
  shared.with_write(|v| *v = 200);
  shared.with_read(|v| assert_eq!(*v, 200));
}

#[test]
fn rwlock_driver_trait_impl() {
  let lock = <CheckedSpinSyncRwLock<u32> as RwLockDriver<u32>>::new(5);
  {
    let guard = RwLockDriver::read(&lock);
    assert_eq!(*guard, 5);
  }
  {
    let mut guard = RwLockDriver::write(&lock);
    *guard = 10;
  }
  assert_eq!(RwLockDriver::into_inner(lock), 10);
}
