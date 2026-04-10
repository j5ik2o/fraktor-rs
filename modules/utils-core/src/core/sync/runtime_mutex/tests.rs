use super::RuntimeMutex;
use crate::core::sync::SpinSyncMutex;

#[test]
fn runtime_mutex_protects_value() {
  let mutex: RuntimeMutex<_> = RuntimeMutex::new(5_u32);
  assert_eq!(*mutex.lock(), 5);
}

#[test]
fn runtime_mutex_explicit_driver_matches_default() {
  let mutex: RuntimeMutex<_, SpinSyncMutex<_>> = RuntimeMutex::new_with_driver(5_u32);
  assert_eq!(*mutex.lock(), 5);
}
