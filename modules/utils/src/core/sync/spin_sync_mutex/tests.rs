use super::SpinSyncMutex;

#[test]
fn test_spin_sync_mutex_as_inner() {
  let mutex = SpinSyncMutex::new(42i32);
  let inner = mutex.as_inner();

  // Use the inner mutex to verify it's the same one
  let guard = inner.lock();
  assert_eq!(*guard, 42);
}

#[test]
fn test_spin_sync_mutex_new_and_lock() {
  let mutex = SpinSyncMutex::new("test");
  let guard = mutex.lock();
  assert_eq!(*guard, "test");
}

#[test]
fn test_spin_sync_mutex_into_inner() {
  let mutex = SpinSyncMutex::new(vec![1, 2, 3]);
  let inner_vec = mutex.into_inner();
  assert_eq!(inner_vec, vec![1, 2, 3]);
}

#[test]
fn test_spin_sync_mutex_multiple_locks() {
  let mutex = SpinSyncMutex::new(0i32);

  {
    let mut guard = mutex.lock();
    *guard = 42;
  }

  {
    let guard = mutex.lock();
    assert_eq!(*guard, 42);
  }
}

#[test]
fn test_spin_sync_mutex_inherent_lock() {
  let mutex = SpinSyncMutex::new("hello");

  // Inherent methods are reachable without any trait import.
  {
    let guard = mutex.lock();
    assert_eq!(*guard, "hello");
  }

  let value = mutex.into_inner();
  assert_eq!(value, "hello");
}

#[test]
fn test_spin_sync_mutex_const_new() {
  static MUTEX: SpinSyncMutex<i32> = SpinSyncMutex::new(123);
  let guard = MUTEX.lock();
  assert_eq!(*guard, 123);
}
