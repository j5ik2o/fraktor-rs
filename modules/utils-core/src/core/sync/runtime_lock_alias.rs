//! Runtime-selected lock aliases shared across the crate.

#[cfg(test)]
mod tests;

use crate::core::sync::{RuntimeMutex, SpinSyncMutex};

/// No-std mutex alias.
pub type NoStdMutex<T> = RuntimeMutex<T, SpinSyncMutex<T>>;
