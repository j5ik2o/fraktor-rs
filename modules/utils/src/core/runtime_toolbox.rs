//! Runtime toolbox abstraction selecting synchronization primitives and time providers.

use crate::core::time::{MonotonicClock, SchedulerTickHandle};

#[cfg(test)]
mod tests;

mod no_std_toolbox;

pub use no_std_toolbox::NoStdToolbox;

/// Provides access to synchronization primitives required by the runtime.
pub trait RuntimeToolbox: Send + Sync + 'static {
  /// Clock implementation exposed through the toolbox.
  type Clock: MonotonicClock;

  /// Returns the monotonic clock.
  fn clock(&self) -> &Self::Clock;

  /// Creates a tick handle scoped to this toolbox.
  fn tick_source(&self) -> SchedulerTickHandle<'_>;
}

pub type RuntimeMutex<T> = crate::RuntimeMutexBackend<T>;

pub type RuntimeRwLock<T> = crate::RuntimeRwLockBackend<T>;

pub type NoStdMutex<T> = RuntimeMutex<T>;

pub type NoStdRwLock<T> = RuntimeRwLock<T>;
