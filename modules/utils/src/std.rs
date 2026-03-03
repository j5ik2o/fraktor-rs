/// Collection utilities for std environments.
pub mod collections;
/// Synchronization primitives built on top of `std::sync::Mutex`.
mod sync_mutex;
/// Guard returned by [`StdSyncMutex`](crate::StdSyncMutex).
mod sync_mutex_guard;
/// Synchronization primitives built on top of `std::sync::RwLock`.
mod sync_rwlock;
/// Guard returned by [`StdSyncRwLock::read`](crate::StdSyncRwLock::read).
mod sync_rwlock_read_guard;
/// Guard returned by [`StdSyncRwLock::write`](crate::StdSyncRwLock::write).
mod sync_rwlock_write_guard;

pub use sync_mutex::StdSyncMutex;
pub use sync_mutex_guard::StdSyncMutexGuard;
pub use sync_rwlock::StdSyncRwLock;
pub use sync_rwlock_read_guard::StdSyncRwLockReadGuard;
pub use sync_rwlock_write_guard::StdSyncRwLockWriteGuard;

/// Convenience alias for the default std mutex.
pub type StdMutex<T> = crate::core::sync::RuntimeMutex<T>;
