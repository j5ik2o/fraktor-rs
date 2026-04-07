use std::{
  sync::Arc,
  thread,
  time::{Duration, Instant},
};

use super::DebugSpinSyncMutex;

#[test]
fn lock_returns_guard_with_value() {
  let mutex = DebugSpinSyncMutex::new(42_u32);
  let guard = mutex.lock();
  assert_eq!(*guard, 42);
}

#[test]
fn guard_supports_mutation() {
  let mutex = DebugSpinSyncMutex::new(0_u32);
  {
    let mut guard = mutex.lock();
    *guard = 100;
  }
  let guard = mutex.lock();
  assert_eq!(*guard, 100);
}

#[test]
fn into_inner_yields_value() {
  let mutex = DebugSpinSyncMutex::new(String::from("hello"));
  let value = mutex.into_inner();
  assert_eq!(value, "hello");
}

#[test]
#[should_panic(expected = "DebugSpinSyncMutex re-entered by the same thread")]
fn re_entry_on_same_thread_panics() {
  let mutex = DebugSpinSyncMutex::new(0_u32);
  let _g1 = mutex.lock();
  let _g2 = mutex.lock(); // panics
}

#[test]
fn dropping_guard_clears_owner_so_subsequent_lock_succeeds() {
  let mutex = DebugSpinSyncMutex::new(0_u32);
  {
    let _g1 = mutex.lock();
  }
  let _g2 = mutex.lock(); // ok, previous guard dropped
}

#[test]
fn contention_from_other_thread_does_not_panic() {
  // Thread A holds the lock, thread B waits and acquires after release.
  // This must NOT panic — only same-thread re-entry is the bug.
  let mutex = Arc::new(DebugSpinSyncMutex::new(0_u32));
  let a = Arc::clone(&mutex);
  let handle = thread::spawn(move || {
    let mut guard = a.lock();
    thread::sleep(Duration::from_millis(50));
    *guard = 1;
  });
  // Give thread A a chance to acquire first.
  thread::sleep(Duration::from_millis(10));
  // This blocks (spin) until thread A releases.
  let started = Instant::now();
  let guard = mutex.lock();
  assert!(started.elapsed() >= Duration::from_millis(20));
  assert_eq!(*guard, 1);
  drop(guard);
  handle.join().expect("worker thread");
}

#[test]
fn many_sequential_locks_on_same_thread_are_fine() {
  // Acquire and release many times in sequence on a single thread.
  // Re-entry detection must not produce false positives.
  let mutex = DebugSpinSyncMutex::new(0_u32);
  for i in 0..100 {
    let mut guard = mutex.lock();
    *guard = i;
    drop(guard);
  }
  assert_eq!(*mutex.lock(), 99);
}
