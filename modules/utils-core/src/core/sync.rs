#[allow(clippy::disallowed_types)]
mod arc_shared;
mod lock_driver;
mod lock_driver_factory;
/// Runtime-selected lock type aliases.
mod runtime_lock_alias;
/// Runtime-selected mutex surface.
mod runtime_mutex;
/// Runtime-selected rwlock surface.
mod runtime_rw_lock;
mod rw_lock_driver_factory;
mod rwlock_driver;
pub mod shared;
mod shared_access;
mod shared_error;
mod shared_lock;
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

pub use arc_shared::ArcShared;
pub use lock_driver::LockDriver;
pub use lock_driver_factory::LockDriverFactory;
pub use runtime_lock_alias::NoStdMutex;
pub use runtime_mutex::RuntimeMutex;
pub use runtime_rw_lock::RuntimeRwLock;
pub use rw_lock_driver_factory::RwLockDriverFactory;
pub use rwlock_driver::RwLockDriver;
pub use shared_access::SharedAccess;
pub use shared_error::SharedError;
pub use shared_lock::SharedLock;
pub use spin_sync_factory::SpinSyncFactory;
pub use spin_sync_mutex::SpinSyncMutex;
pub use spin_sync_rwlock::SpinSyncRwLock;
pub use spin_sync_rwlock_factory::SpinSyncRwLockFactory;
pub use weak_shared::WeakShared;
