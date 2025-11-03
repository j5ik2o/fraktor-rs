//! Runtime toolbox abstraction selecting synchronization families.

#[cfg(test)]
mod tests;

mod no_std_toolbox;

pub use no_std_toolbox::NoStdToolbox;

use super::mutex_family::SyncMutexFamily;

/// Provides access to synchronization primitives required by the runtime.
pub trait RuntimeToolbox: Send + Sync + 'static {
  /// Mutex family used to instantiate synchronization primitives.
  type MutexFamily: SyncMutexFamily;
}

/// Helper alias exposing the mutex type produced by the selected toolbox.
pub type ToolboxMutex<T, TB> = <<TB as RuntimeToolbox>::MutexFamily as SyncMutexFamily>::Mutex<T>;

/// Convenience alias for the default no_std mutex.
pub type NoStdMutex<T> = ToolboxMutex<T, NoStdToolbox>;
