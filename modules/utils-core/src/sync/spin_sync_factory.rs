use super::{LockDriverFactory, SpinSyncMutex};

/// Default no-std mutex driver family based on [`SpinSyncMutex`].
pub struct SpinSyncFactory;

impl LockDriverFactory for SpinSyncFactory {
  type Driver<T> = SpinSyncMutex<T>;
}
