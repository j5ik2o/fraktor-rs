#[allow(clippy::disallowed_types)]
mod arc_shared;
mod default_lock_driver;
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
pub use default_lock_driver::{DefaultLockDriver, DefaultRwLockDriver};
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
