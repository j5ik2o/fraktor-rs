#[allow(clippy::disallowed_types)]
mod arc_shared;
/// Runtime-selected lock type aliases.
mod runtime_lock_alias;
pub mod shared;
mod shared_access;
mod shared_error;
/// Spin-based mutex wrapper used as the canonical sync primitive.
#[allow(clippy::disallowed_types)]
mod spin_sync_mutex;
/// Spin-based read-write lock wrapper used as the canonical sync primitive.
#[allow(clippy::disallowed_types)]
mod spin_sync_rwlock;
#[allow(clippy::disallowed_types)]
mod weak_shared;

pub use arc_shared::ArcShared;
pub use runtime_lock_alias::{NoStdMutex, RuntimeMutex, RuntimeRwLock};
pub use shared_access::SharedAccess;
pub use shared_error::SharedError;
pub use spin_sync_mutex::SpinSyncMutex;
pub use spin_sync_rwlock::SpinSyncRwLock;
pub use weak_shared::WeakShared;
