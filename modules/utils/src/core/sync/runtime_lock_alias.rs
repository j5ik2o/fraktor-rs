//! Runtime-selected lock type aliases shared across the crate.

#[cfg(test)]
mod tests;

/// Runtime-selected mutex alias.
pub type RuntimeMutex<T> = crate::RuntimeMutexBackend<T>;

/// Runtime-selected rwlock alias.
pub type RuntimeRwLock<T> = crate::RuntimeRwLockBackend<T>;

/// No-std mutex alias.
pub type NoStdMutex<T> = RuntimeMutex<T>;
