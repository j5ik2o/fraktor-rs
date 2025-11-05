#![deny(missing_docs)]
#![deny(unreachable_pub)]

//! Standard library extensions for Cellactor utilities.

/// Runtime toolbox and aliases for std environments.
pub mod runtime_toolbox;
/// Synchronization primitives built on top of `std::sync::Mutex`.
mod sync_mutex;
mod sync_mutex_guard;

pub use sync_mutex::StdSyncMutex;
pub use sync_mutex_guard::StdSyncMutexGuard;
