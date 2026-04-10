use super::DebugSpinSyncMutex;

#[test]
fn locks_and_mutates_value() {
  let mutex = DebugSpinSyncMutex::new(1_u32);
  *mutex.lock() = 2;
  assert_eq!(*mutex.lock(), 2);
}

#[test]
#[should_panic(expected = "DebugSpinSyncMutex detected same-thread re-entry")]
fn panics_on_same_thread_reentry() {
  let mutex = DebugSpinSyncMutex::new(1_u32);
  let _guard = mutex.lock();
  let _reenter = mutex.lock();
}
