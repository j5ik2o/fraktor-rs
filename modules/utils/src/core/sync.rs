#[allow(clippy::disallowed_types)]
mod arc_shared;
/// Async-aware mutex abstractions shared across runtimes.
pub(crate) mod async_mutex_like;
#[allow(clippy::disallowed_types)]
mod flag;
/// Helper traits for shared function and factory closures.
pub(crate) mod function;
/// Policies for detecting interrupt contexts prior to blocking operations.
pub(crate) mod interrupt;
#[cfg(feature = "alloc")]
#[allow(clippy::disallowed_types)]
mod rc_shared;
/// Runtime-selected lock type aliases.
mod runtime_lock_alias;
pub mod shared;
mod shared_access;
mod shared_error;
mod state;
mod static_ref_shared;
/// Synchronous mutex abstractions shared across runtimes.
pub mod sync_mutex_like;
/// Synchronous read-write lock abstractions shared across runtimes.
pub mod sync_rwlock_like;
#[allow(clippy::disallowed_types)]
mod weak_shared;

pub use arc_shared::ArcShared;
#[allow(unused_imports)]
pub(crate) use flag::Flag;
#[cfg(feature = "alloc")]
#[allow(unused_imports)]
pub(crate) use rc_shared::RcShared;
pub use runtime_lock_alias::{NoStdMutex, RuntimeMutex, RuntimeRwLock};
pub use shared_access::SharedAccess;
pub use shared_error::SharedError;
#[allow(unused_imports)]
pub(crate) use state::StateCell;
#[allow(unused_imports)]
pub(crate) use static_ref_shared::StaticRefShared;
pub use weak_shared::WeakShared;
