extern crate alloc;
use alloc::boxed::Box;

use fraktor_utils_rs::{
  core::sync::ArcShared,
  std::{StdSyncMutex, runtime_toolbox::StdToolbox},
};
#[cfg(feature = "tokio-executor")]
use tokio::runtime::Handle;

#[cfg(feature = "tokio-executor")]
use super::dispatch_executor::TokioExecutor;
use super::{DispatchExecutor, DispatchExecutorAdapter, DispatcherShared, StdScheduleAdapter};
use crate::core::{
  dispatch::{
    dispatcher::{
      DispatchExecutorRunnerGeneric, DispatcherConfigGeneric as CoreDispatcherConfigGeneric,
      ScheduleAdapterSharedGeneric,
    },
    mailbox::MailboxGeneric,
  },
  spawn::SpawnError,
};

#[cfg(all(test, feature = "tokio-executor"))]
mod tests;

/// Dispatcher configuration specialised for `StdToolbox`.
#[derive(Clone, Default)]
pub struct DispatcherConfig {
  inner: CoreDispatcherConfigGeneric<StdToolbox>,
}

impl DispatcherConfig {
  /// Creates a configuration from a scheduler implementation.
  ///
  /// The executor is wrapped in a `StdSyncMutex` for external synchronization.
  #[must_use]
  pub fn from_executor(executor: ArcShared<StdSyncMutex<Box<dyn DispatchExecutor>>>) -> Self {
    let executor_adapter = Box::new(DispatchExecutorAdapter::new(executor));
    let schedule_adapter = ScheduleAdapterSharedGeneric::<StdToolbox>::new(Box::new(StdScheduleAdapter::default()));
    let inner = CoreDispatcherConfigGeneric::from_executor(executor_adapter).with_schedule_adapter(schedule_adapter);
    Self { inner }
  }

  /// Creates a configuration from the current Tokio runtime handle.
  ///
  /// # Panics
  ///
  /// Panics when called outside a Tokio runtime context.
  #[cfg(feature = "tokio-executor")]
  #[must_use]
  pub fn tokio_auto() -> Self {
    let Ok(handle) = Handle::try_current() else {
      panic!("Tokio runtime handle unavailable");
    };
    Self::from_executor(ArcShared::new(StdSyncMutex::new(Box::new(TokioExecutor::new(handle)))))
  }

  /// Returns the configured scheduler runner.
  ///
  /// The returned [`DispatchExecutorRunnerGeneric`] implements [`DispatchExecutor`] and can be used
  /// to submit dispatchers for execution.
  #[must_use]
  pub fn executor(&self) -> ArcShared<DispatchExecutorRunnerGeneric<StdToolbox>> {
    self.inner.executor()
  }

  /// Builds a dispatcher using the configured scheduler.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError::InvalidMailboxConfig`] if the mailbox configuration is incompatible
  /// with the executor (e.g., using Block strategy with a non-blocking executor).
  pub fn build_dispatcher(
    &self,
    mailbox: ArcShared<MailboxGeneric<StdToolbox>>,
  ) -> Result<DispatcherShared, SpawnError> {
    self.inner.build_dispatcher(mailbox)
  }

  /// Borrows the underlying core configuration.
  #[must_use]
  pub const fn as_core(&self) -> &CoreDispatcherConfigGeneric<StdToolbox> {
    &self.inner
  }

  /// Consumes the wrapper and returns the core configuration.
  #[must_use]
  pub fn into_core(self) -> CoreDispatcherConfigGeneric<StdToolbox> {
    self.inner
  }

  /// Wraps an existing core configuration.
  #[must_use]
  pub const fn from_core(inner: CoreDispatcherConfigGeneric<StdToolbox>) -> Self {
    Self { inner }
  }
}
