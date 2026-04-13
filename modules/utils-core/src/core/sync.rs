#[allow(clippy::disallowed_types)]
mod arc_shared;
/// Read guard for the checked spin rwlock.
mod checked_rw_lock_read_guard;
/// Write guard for the checked spin rwlock.
mod checked_rw_lock_write_guard;
/// Re-entry detecting spin mutex (no_std compatible).
#[allow(clippy::disallowed_types)]
mod checked_spin_sync_mutex;
/// Guard for the checked spin mutex.
mod checked_spin_sync_mutex_guard;
/// Re-entry detecting spin rwlock (no_std compatible).
#[allow(clippy::disallowed_types)]
mod checked_spin_sync_rwlock;
mod lock_driver;
mod lock_driver_factory;
mod rw_lock_driver_factory;
mod rwlock_driver;
pub mod shared;
mod shared_access;
mod shared_error;
mod shared_lock;
mod shared_rw_lock;
mod spin_sync_factory;
/// Spin-based mutex wrapper used as the canonical sync primitive.
#[allow(clippy::disallowed_types)]
mod spin_sync_mutex;
/// Spin-based read-write lock wrapper used as the canonical sync primitive.
#[allow(clippy::disallowed_types)]
mod spin_sync_rwlock;
mod spin_sync_rwlock_factory;
#[allow(clippy::disallowed_types)]
mod weak_shared;
mod weak_shared_lock;
mod weak_shared_rw_lock;

pub use arc_shared::ArcShared;
pub use checked_rw_lock_read_guard::CheckedRwLockReadGuard;
pub use checked_rw_lock_write_guard::CheckedRwLockWriteGuard;
pub use checked_spin_sync_mutex::CheckedSpinSyncMutex;
pub use checked_spin_sync_mutex_guard::CheckedSpinSyncMutexGuard;
pub use checked_spin_sync_rwlock::CheckedSpinSyncRwLock;
pub use lock_driver::LockDriver;
pub use lock_driver_factory::LockDriverFactory;
pub use rw_lock_driver_factory::RwLockDriverFactory;
pub use rwlock_driver::RwLockDriver;
pub use shared_access::SharedAccess;
pub use shared_error::SharedError;
pub use shared_lock::SharedLock;
pub use shared_rw_lock::SharedRwLock;
pub use spin_sync_factory::SpinSyncFactory;
pub use spin_sync_mutex::SpinSyncMutex;
pub use spin_sync_rwlock::SpinSyncRwLock;
pub use spin_sync_rwlock_factory::SpinSyncRwLockFactory;
pub use weak_shared::WeakShared;
pub use weak_shared_lock::WeakSharedLock;
pub use weak_shared_rw_lock::WeakSharedRwLock;

/// Default mutex backend. Resolves to [`SpinSyncMutex`] in production
/// and [`CheckedSpinSyncMutex`] when the `debug-locks` feature is enabled.
#[cfg(not(feature = "debug-locks"))]
pub type DefaultMutex<T> = SpinSyncMutex<T>;
/// Default mutex backend with re-entry detection (debug-locks enabled).
#[cfg(feature = "debug-locks")]
pub type DefaultMutex<T> = CheckedSpinSyncMutex<T>;

/// Default rwlock backend. Resolves to [`SpinSyncRwLock`] in production
/// and [`CheckedSpinSyncRwLock`] when the `debug-locks` feature is enabled.
#[cfg(not(feature = "debug-locks"))]
pub type DefaultRwLock<T> = SpinSyncRwLock<T>;
/// Default rwlock backend with re-entry detection (debug-locks enabled).
#[cfg(feature = "debug-locks")]
pub type DefaultRwLock<T> = CheckedSpinSyncRwLock<T>;
