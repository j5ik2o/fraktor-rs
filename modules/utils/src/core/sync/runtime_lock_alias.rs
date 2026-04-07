//! Runtime-selected lock type aliases shared across the crate.

#[cfg(test)]
mod tests;

use crate::core::sync::{SpinSyncMutex, SpinSyncRwLock};

/// Runtime-selected mutex alias.
pub type RuntimeMutex<T> = SpinSyncMutex<T>;

/// Runtime-selected rwlock alias.
pub type RuntimeRwLock<T> = SpinSyncRwLock<T>;

/// No-std mutex alias.
pub type NoStdMutex<T> = RuntimeMutex<T>;
