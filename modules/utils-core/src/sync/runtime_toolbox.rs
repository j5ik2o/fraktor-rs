//! Runtime toolbox abstraction selecting synchronization families.

#[cfg(feature = "std")]
use super::mutex_family::StdMutexFamily;
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

/// Toolbox for std environments, backed by [`StdMutexFamily`].
#[cfg(feature = "std")]
#[derive(Clone, Copy, Debug, Default)]
#[allow(dead_code)]
pub struct StdToolbox;

#[cfg(feature = "std")]
impl RuntimeToolbox for StdToolbox {
  type MutexFamily = StdMutexFamily;
}

/// Convenience alias for the default std mutex.
#[cfg(feature = "std")]
#[allow(dead_code)]
pub type StdMutex<T> = ToolboxMutex<T, StdToolbox>;

#[cfg(test)]
mod tests;
