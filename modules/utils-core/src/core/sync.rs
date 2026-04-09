#[allow(clippy::disallowed_types)]
mod arc_shared;
mod lock_driver;
mod lock_driver_factory;
/// Runtime-selected lock type aliases.
mod runtime_lock_alias;
mod rwlock_driver;
pub mod shared;
mod shared_access;
mod shared_error;
mod spin_sync_factory;
/// Spin-based mutex wrapper used as the canonical sync primitive.
#[allow(clippy::disallowed_types)]
mod spin_sync_mutex;
mod spin_sync_rwlock_factory;
/// Spin-based read-write lock wrapper used as the canonical sync primitive.
#[allow(clippy::disallowed_types)]
mod spin_sync_rwlock;
#[allow(clippy::disallowed_types)]
mod weak_shared;

pub use arc_shared::ArcShared;
pub use lock_driver::LockDriver;
pub use lock_driver_factory::{LockDriverFactory, RwLockDriverFactory};
pub use runtime_lock_alias::{NoStdMutex, RuntimeMutex, RuntimeRwLock};
pub use rwlock_driver::RwLockDriver;
pub use shared_access::SharedAccess;
pub use shared_error::SharedError;
pub use spin_sync_factory::SpinSyncFactory;
pub use spin_sync_mutex::SpinSyncMutex;
pub use spin_sync_rwlock_factory::SpinSyncRwLockFactory;
pub use spin_sync_rwlock::SpinSyncRwLock;
pub use weak_shared::WeakShared;
