#![deny(missing_docs)]

//! Standard library extensions for Cellactor utilities.

mod std_toolbox;
/// Synchronization primitives built on top of `std::sync::Mutex`.
mod sync_mutex;
mod sync_mutex_guard;
/// Runtime toolbox and aliases for std environments.
mod toolbox;

pub use std_toolbox::StdToolbox;
pub use sync_mutex::StdSyncMutex;
pub use sync_mutex_guard::StdSyncMutexGuard;
pub use toolbox::{StdMutex, StdMutexFamily};
