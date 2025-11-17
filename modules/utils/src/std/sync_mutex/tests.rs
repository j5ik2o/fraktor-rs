use super::StdSyncMutex;
use crate::core::sync::sync_mutex_like::SyncMutexLike;

#[test]
fn new_creates_mutex() {
  let mutex = StdSyncMutex::new(42);
  let guard = mutex.lock();
  assert_eq!(*guard, 42);
}

#[test]
fn lock_allows_read_write() {
  let mutex = StdSyncMutex::new(0);
  {
    let mut guard = mutex.lock();
    *guard = 123;
  }
  let guard = mutex.lock();
  assert_eq!(*guard, 123);
}

#[test]
fn into_inner_unwraps_value() {
  let mutex = StdSyncMutex::new(999);
  let value = mutex.into_inner();
  assert_eq!(value, 999);
}

#[test]
fn sync_mutex_like_new() {
  let mutex = <StdSyncMutex<i32> as SyncMutexLike<i32>>::new(55);
  let guard = mutex.lock();
  assert_eq!(*guard, 55);
}

#[test]
fn sync_mutex_like_into_inner() {
  let mutex = <StdSyncMutex<i32> as SyncMutexLike<i32>>::new(77);
  let value = <StdSyncMutex<i32> as SyncMutexLike<i32>>::into_inner(mutex);
  assert_eq!(value, 77);
}

#[test]
fn sync_mutex_like_lock() {
  let mutex = <StdSyncMutex<i32> as SyncMutexLike<i32>>::new(88);
  let guard = <StdSyncMutex<i32> as SyncMutexLike<i32>>::lock(&mutex);
  assert_eq!(*guard, 88);
}
