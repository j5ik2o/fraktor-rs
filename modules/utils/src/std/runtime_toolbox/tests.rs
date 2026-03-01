use super::{StdMutex, StdRwLock};
use crate::{
  core::{
    runtime_toolbox::{RuntimeMutex, RuntimeRwLock},
    sync::{sync_mutex_like::SyncMutexLike, sync_rwlock_like::SyncRwLockLike},
  },
  std::{StdSyncMutex, StdSyncRwLock},
};

#[test]
fn std_mutex_creates_mutex() {
  let mutex = StdMutex::new(42);
  let guard = mutex.lock();
  assert_eq!(*guard, 42);
}

#[test]
fn std_mutex_is_independent() {
  let mutex1 = StdMutex::new(100);
  let mutex2 = StdMutex::new(200);
  assert_eq!(*mutex1.lock(), 100);
  assert_eq!(*mutex2.lock(), 200);
}

#[test]
fn std_mutex_default_value() {
  let mutex = StdMutex::new(999);
  assert_eq!(*mutex.lock(), 999);
}

#[test]
fn std_mutex_type_alias_works() {
  let mutex: StdMutex<i32> = StdMutex::new(123);
  let guard = mutex.lock();
  assert_eq!(*guard, 123);
}

#[test]
fn std_rwlock_creates_lock() {
  let lock = StdRwLock::new(7);
  assert_eq!(*lock.read(), 7);
}

#[test]
fn std_rwlock_type_alias_works() {
  let lock: StdRwLock<i32> = StdRwLock::new(1);
  {
    let mut guard = lock.write();
    *guard = 2;
  }
  assert_eq!(*lock.read(), 2);
}

#[test]
fn runtime_lock_aliases_use_std_backend() {
  let _: RuntimeMutex<u8> = StdSyncMutex::new(1);
  let _: RuntimeRwLock<u8> = StdSyncRwLock::new(1);
}
