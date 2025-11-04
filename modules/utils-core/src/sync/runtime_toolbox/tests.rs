use super::{NoStdToolbox, RuntimeToolbox, ToolboxMutex};
use crate::sync::mutex_family::SyncMutexFamily;

#[test]
fn toolbox_mutex_uses_spin_family() {
  type Family = <NoStdToolbox as RuntimeToolbox>::MutexFamily;
  let mutex: ToolboxMutex<_, NoStdToolbox> = Family::create(5_u32);
  assert_eq!(*mutex.lock(), 5);
}
