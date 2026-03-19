#[allow(clippy::disallowed_types)]
mod arc_shared;
/// Runtime-selected lock type aliases.
mod runtime_lock_alias;
pub mod shared;
mod shared_access;
mod shared_error;
/// Synchronous mutex abstractions shared across runtimes.
pub mod sync_mutex_like;
/// Synchronous read-write lock abstractions shared across runtimes.
pub mod sync_rwlock_like;
#[allow(clippy::disallowed_types)]
mod weak_shared;

pub use arc_shared::ArcShared;
pub use runtime_lock_alias::{NoStdMutex, RuntimeMutex, RuntimeRwLock};
pub use shared_access::SharedAccess;
pub use shared_error::SharedError;
pub use weak_shared::WeakShared;
