use crate::core::runtime_toolbox::SyncMutexFamily;

use super::{StdMutex, StdMutexFamily};

#[test]
fn std_mutex_family_creates_mutex() {
  let mutex = <StdMutexFamily as SyncMutexFamily>::create(42);
  let guard = mutex.lock();
  assert_eq!(*guard, 42);
}

#[test]
fn std_mutex_family_is_debug() {
  let family = StdMutexFamily;
  let debug_str = format!("{:?}", family);
  assert!(debug_str.contains("StdMutexFamily"));
}

#[test]
fn std_mutex_family_is_clone() {
  let mutex1 = <StdMutexFamily as SyncMutexFamily>::create(100);
  let mutex2 = <StdMutexFamily as SyncMutexFamily>::create(200);
  assert_eq!(*mutex1.lock(), 100);
  assert_eq!(*mutex2.lock(), 200);
}

#[test]
fn std_mutex_family_default() {
  let mutex = <StdMutexFamily as SyncMutexFamily>::create(999);
  assert_eq!(*mutex.lock(), 999);
}

#[test]
fn std_mutex_type_alias_works() {
  let mutex: StdMutex<i32> = <StdMutexFamily as SyncMutexFamily>::create(123);
  let guard = mutex.lock();
  assert_eq!(*guard, 123);
}
