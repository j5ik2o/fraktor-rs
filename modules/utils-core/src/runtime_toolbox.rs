//! Runtime toolbox abstraction selecting synchronization families and time primitives.

use crate::time::{MonotonicClock, SchedulerTickHandle};

#[cfg(test)]
mod tests;

mod no_std_toolbox;
pub mod sync_mutex_family;

pub use no_std_toolbox::NoStdToolbox;
pub use sync_mutex_family::{SpinMutexFamily, SyncMutexFamily};

/// Provides access to synchronization primitives required by the runtime.
pub trait RuntimeToolbox: Send + Sync + 'static {
  /// Mutex family used to instantiate synchronization primitives.
  type MutexFamily: SyncMutexFamily;
  /// Clock implementation exposed through the toolbox.
  type Clock: MonotonicClock;

  /// Returns the monotonic clock.
  fn clock(&self) -> &Self::Clock;

  /// Creates a tick handle scoped to this toolbox.
  fn tick_source(&self) -> SchedulerTickHandle<'_>;
}

/// Helper alias exposing the mutex type produced by the selected toolbox.
pub type ToolboxMutex<T, TB> = <<TB as RuntimeToolbox>::MutexFamily as SyncMutexFamily>::Mutex<T>;

/// Convenience alias for the default no_std mutex.
pub type NoStdMutex<T> = ToolboxMutex<T, NoStdToolbox>;
