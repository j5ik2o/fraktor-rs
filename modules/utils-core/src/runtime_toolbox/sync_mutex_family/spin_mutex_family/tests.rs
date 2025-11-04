use super::SpinMutexFamily;
use crate::runtime_toolbox::sync_mutex_family::SyncMutexFamily;

#[test]
fn spin_mutex_family_creates_functional_mutex() {
  let mutex = <SpinMutexFamily as SyncMutexFamily>::create(1_u32);
  {
    let mut guard = mutex.lock();
    *guard += 1;
  }
  assert_eq!(*mutex.lock(), 2);
}
