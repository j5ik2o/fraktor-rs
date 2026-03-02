use crate::core::runtime_toolbox::{RuntimeMutex, RuntimeRwLock};

mod std_toolbox;
#[cfg(test)]
mod tests;

pub use std_toolbox::StdToolbox;

/// Convenience alias for the default std mutex.
pub type StdMutex<T> = RuntimeMutex<T>;
/// Convenience alias for the default std rwlock.
pub type StdRwLock<T> = RuntimeRwLock<T>;
