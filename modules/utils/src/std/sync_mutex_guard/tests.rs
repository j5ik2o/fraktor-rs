use alloc::string::String;

use crate::std::sync_mutex::StdSyncMutex;

#[test]
fn deref_allows_read() {
  let mutex = StdSyncMutex::new(42);
  let guard = mutex.lock();
  assert_eq!(*guard, 42);
}

#[test]
fn deref_mut_allows_write() {
  let mutex = StdSyncMutex::new(0);
  {
    let mut guard = mutex.lock();
    *guard = 123;
  }
  let guard = mutex.lock();
  assert_eq!(*guard, 123);
}

#[test]
fn guard_implements_deref() {
  let mutex = StdSyncMutex::new(String::from("hello"));
  let guard = mutex.lock();
  assert_eq!(guard.len(), 5);
  assert_eq!(guard.as_str(), "hello");
}

#[test]
fn guard_implements_deref_mut() {
  let mutex = StdSyncMutex::new(String::from("hello"));
  {
    let mut guard = mutex.lock();
    guard.push_str(" world");
  }
  let guard = mutex.lock();
  assert_eq!(guard.as_str(), "hello world");
}

#[test]
fn multiple_guards_can_be_created_sequentially() {
  let mutex = StdSyncMutex::new(100);
  {
    let guard1 = mutex.lock();
    assert_eq!(*guard1, 100);
  }
  {
    let guard2 = mutex.lock();
    assert_eq!(*guard2, 100);
  }
}
