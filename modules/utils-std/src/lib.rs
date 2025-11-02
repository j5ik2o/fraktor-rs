#![deny(missing_docs)]

//! Standard library extensions for Cellactor utilities.

/// Synchronization primitives built on top of `std::sync::Mutex`.
pub mod sync_mutex;
/// Runtime toolbox and aliases for std environments.
pub mod toolbox;

pub use sync_mutex::{StdSyncMutex, StdSyncMutexGuard};
pub use toolbox::{StdMutex, StdMutexFamily, StdToolbox};
