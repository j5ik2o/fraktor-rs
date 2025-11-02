//! Runtime toolbox abstraction selecting synchronization families.

use super::mutex_family::{SpinMutexFamily, SyncMutexFamily};

/// Provides access to synchronization primitives required by the runtime.
pub trait RuntimeToolbox: Send + Sync + 'static {
  /// Mutex family used to instantiate synchronization primitives.
  type MutexFamily: SyncMutexFamily;
}

/// Default toolbox for no_std environments, backed by [`SpinMutexFamily`].
#[derive(Clone, Copy, Debug, Default)]
pub struct NoStdToolbox;

impl RuntimeToolbox for NoStdToolbox {
  type MutexFamily = SpinMutexFamily;
}

/// Helper alias exposing the mutex type produced by the selected toolbox.
pub type ToolboxMutex<T, TB> = <<TB as RuntimeToolbox>::MutexFamily as SyncMutexFamily>::Mutex<T>;

/// Convenience alias for the default no_std mutex.
pub type NoStdMutex<T> = ToolboxMutex<T, NoStdToolbox>;

#[cfg(test)]
mod tests;
