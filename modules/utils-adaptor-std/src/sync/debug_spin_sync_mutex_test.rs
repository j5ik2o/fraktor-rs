use super::{DebugSpinSyncMutex, current_thread_id_u64};

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

#[test]
fn thread_id_hash_is_never_zero() {
  // The sentinel value 0 means "no owner". The hash must never collide with it.
  assert_ne!(current_thread_id_u64(), 0);
}
