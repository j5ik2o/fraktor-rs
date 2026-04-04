//! Runtime-selected lock type aliases shared across the crate.

#[cfg(test)]
mod tests;

use crate::{RuntimeMutexBackend, RuntimeRwLockBackend};

/// Runtime-selected mutex alias.
pub type RuntimeMutex<T> = RuntimeMutexBackend<T>;

/// Runtime-selected rwlock alias.
pub type RuntimeRwLock<T> = RuntimeRwLockBackend<T>;

/// No-std mutex alias.
pub type NoStdMutex<T> = RuntimeMutex<T>;
