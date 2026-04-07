use std::{sync::Arc, thread};

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
fn heavy_contention_from_many_threads_does_not_panic() {
  // Verifies that simultaneous lock contention from different threads
  // never trips the same-thread re-entry assertion. Each worker performs
  // a tight `lock -> increment -> release` loop, so the lock changes
  // hands many times across many distinct ThreadIds. If `lock()` ever
  // mistakenly fired its `assert_ne!(prior, current)` for a different
  // thread, this test would panic.
  const WORKERS: u32 = 16;
  const ITERATIONS: u32 = 200;
  let mutex = Arc::new(DebugSpinSyncMutex::new(0_u32));
  let mut handles = Vec::with_capacity(WORKERS as usize);
  for _ in 0..WORKERS {
    let m = Arc::clone(&mutex);
    handles.push(thread::spawn(move || {
      for _ in 0..ITERATIONS {
        let mut guard = m.lock();
        *guard += 1;
      }
    }));
  }
  for handle in handles {
    handle.join().expect("worker thread");
  }
  let guard = mutex.lock();
  assert_eq!(*guard, WORKERS * ITERATIONS);
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
