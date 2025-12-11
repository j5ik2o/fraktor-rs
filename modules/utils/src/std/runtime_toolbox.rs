use crate::core::runtime_toolbox::{ToolboxMutex, ToolboxRwLock};

mod std_mutex_family;
mod std_rwlock_family;
mod std_toolbox;
#[cfg(test)]
mod tests;

pub use std_mutex_family::StdMutexFamily;
pub use std_rwlock_family::StdRwLockFamily;
pub use std_toolbox::StdToolbox;

/// Convenience alias for the default std mutex.
pub type StdMutex<T> = ToolboxMutex<T, StdToolbox>;
/// Convenience alias for the default std rwlock.
pub type StdRwLock<T> = ToolboxRwLock<T, StdToolbox>;
