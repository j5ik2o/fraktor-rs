use super::{RwLockDriverFactory, SpinSyncRwLock};

/// Default no-std rwlock driver family based on [`SpinSyncRwLock`].
pub struct SpinSyncRwLockFactory;

impl RwLockDriverFactory for SpinSyncRwLockFactory {
  type Driver<T> = SpinSyncRwLock<T>;
}
