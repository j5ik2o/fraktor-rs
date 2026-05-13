#[allow(clippy::disallowed_types)]
mod arc_shared;
/// Read guard for the checked spin rwlock.
#[cfg(feature = "debug-locks")]
#[allow(cfg_std_forbid)]
mod checked_rw_lock_read_guard;
/// Write guard for the checked spin rwlock.
#[cfg(feature = "debug-locks")]
#[allow(cfg_std_forbid)]
mod checked_rw_lock_write_guard;
/// Re-entry detecting spin mutex (requires std for thread-id based detection).
#[cfg(feature = "debug-locks")]
#[allow(clippy::disallowed_types, cfg_std_forbid)]
mod checked_spin_sync_mutex;
/// Guard for the checked spin mutex.
#[cfg(feature = "debug-locks")]
#[allow(cfg_std_forbid)]
mod checked_spin_sync_mutex_guard;
/// Re-entry detecting spin rwlock (requires std for thread-id based detection).
#[cfg(feature = "debug-locks")]
#[allow(clippy::disallowed_types, cfg_std_forbid)]
mod checked_spin_sync_rwlock;
mod exclusive_cell;
mod lock_driver;
mod lock_driver_factory;
mod once_driver;
mod rw_lock_driver_factory;
mod rwlock_driver;
pub mod shared;
mod shared_access;
mod shared_error;
mod shared_lock;
mod shared_rw_lock;
/// Spin-based write-once cell wrapper acting as the `spin` backend for `OnceDriver`.
#[allow(clippy::disallowed_types)]
mod spin_once;
mod spin_sync_factory;
/// Spin-based mutex wrapper used as the canonical sync primitive.
#[allow(clippy::disallowed_types)]
mod spin_sync_mutex;
/// Spin-based read-write lock wrapper used as the canonical sync primitive.
#[allow(clippy::disallowed_types)]
mod spin_sync_rwlock;
mod spin_sync_rwlock_factory;
/// Standard-library-backed mutex driver (requires std).
/// `std::sync::Mutex` usage is intentional — this is the std-locks backend (reviewed & approved).
#[cfg(feature = "std-locks")]
#[allow(clippy::disallowed_types, cfg_std_forbid)]
mod std_sync_mutex;
/// Standard-library-backed rwlock driver (requires std).
/// `std::sync::RwLock` usage is intentional — this is the std-locks backend (reviewed & approved).
#[cfg(feature = "std-locks")]
#[allow(clippy::disallowed_types, cfg_std_forbid)]
mod std_sync_rwlock;
mod sync_once;
#[allow(clippy::disallowed_types)]
mod weak_shared;
mod weak_shared_lock;
mod weak_shared_rw_lock;

pub use arc_shared::ArcShared;
#[cfg(feature = "debug-locks")]
pub use checked_rw_lock_read_guard::CheckedRwLockReadGuard;
#[cfg(feature = "debug-locks")]
pub use checked_rw_lock_write_guard::CheckedRwLockWriteGuard;
#[cfg(feature = "debug-locks")]
pub use checked_spin_sync_mutex::CheckedSpinSyncMutex;
#[cfg(feature = "debug-locks")]
pub use checked_spin_sync_mutex_guard::CheckedSpinSyncMutexGuard;
#[cfg(feature = "debug-locks")]
pub use checked_spin_sync_rwlock::CheckedSpinSyncRwLock;
pub use exclusive_cell::ExclusiveCell;
pub use lock_driver::LockDriver;
pub use lock_driver_factory::LockDriverFactory;
pub use once_driver::OnceDriver;
pub use rw_lock_driver_factory::RwLockDriverFactory;
pub use rwlock_driver::RwLockDriver;
pub use shared_access::SharedAccess;
pub use shared_error::SharedError;
pub use shared_lock::SharedLock;
pub use shared_rw_lock::SharedRwLock;
pub use spin_once::SpinOnce;
pub use spin_sync_factory::SpinSyncFactory;
pub use spin_sync_mutex::SpinSyncMutex;
pub use spin_sync_rwlock::SpinSyncRwLock;
pub use spin_sync_rwlock_factory::SpinSyncRwLockFactory;
#[cfg(feature = "std-locks")]
pub use std_sync_mutex::StdSyncMutex;
#[cfg(feature = "std-locks")]
pub use std_sync_rwlock::StdSyncRwLock;
pub use sync_once::SyncOnce;
pub use weak_shared::WeakShared;
pub use weak_shared_lock::WeakSharedLock;
pub use weak_shared_rw_lock::WeakSharedRwLock;

/// Default mutex backend with re-entry detection (debug-locks enabled).
#[cfg(feature = "debug-locks")]
pub type DefaultMutex<T> = CheckedSpinSyncMutex<T>;
/// Default mutex backend backed by [`std::sync::Mutex`] (std-locks enabled, debug-locks disabled).
#[cfg(all(feature = "std-locks", not(feature = "debug-locks")))]
pub type DefaultMutex<T> = StdSyncMutex<T>;
/// Default mutex backend backed by spin lock (no std-locks, no debug-locks).
#[cfg(not(any(feature = "debug-locks", feature = "std-locks")))]
pub type DefaultMutex<T> = SpinSyncMutex<T>;

/// Default rwlock backend with re-entry detection (debug-locks enabled).
#[cfg(feature = "debug-locks")]
pub type DefaultRwLock<T> = CheckedSpinSyncRwLock<T>;
/// Default rwlock backend backed by [`std::sync::RwLock`] (std-locks enabled, debug-locks
/// disabled).
#[cfg(all(feature = "std-locks", not(feature = "debug-locks")))]
pub type DefaultRwLock<T> = StdSyncRwLock<T>;
/// Default rwlock backend backed by spin lock (no std-locks, no debug-locks).
#[cfg(not(any(feature = "debug-locks", feature = "std-locks")))]
pub type DefaultRwLock<T> = SpinSyncRwLock<T>;
