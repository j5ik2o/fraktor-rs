extern crate alloc;
use alloc::boxed::Box;

use fraktor_utils_rs::{core::sync::ArcShared, std::runtime_toolbox::StdToolbox};

use super::{DispatchExecutor, DispatchExecutorAdapter, Dispatcher, StdScheduleAdapter};
use crate::core::{
  dispatcher::{DispatchExecutorRunner, DispatcherConfigGeneric as CoreDispatcherConfigGeneric, ScheduleAdapter},
  mailbox::MailboxGeneric,
  spawn::SpawnError,
};

/// Dispatcher configuration specialised for `StdToolbox`.
#[derive(Clone, Default)]
pub struct DispatcherConfig {
  inner: CoreDispatcherConfigGeneric<StdToolbox>,
}

impl DispatcherConfig {
  /// Creates a configuration from a scheduler implementation.
  #[must_use]
  pub fn from_executor(executor: ArcShared<dyn DispatchExecutor>) -> Self {
    let executor_adapter = Box::new(DispatchExecutorAdapter::new(executor));
    let schedule_adapter: ArcShared<dyn ScheduleAdapter<StdToolbox>> = ArcShared::new(StdScheduleAdapter::default());
    let inner = CoreDispatcherConfigGeneric::from_executor(executor_adapter).with_schedule_adapter(schedule_adapter);
    Self { inner }
  }

  /// Returns the configured scheduler runner.
  ///
  /// The returned [`DispatchExecutorRunner`] implements [`DispatchExecutor`] and can be used
  /// to submit dispatchers for execution.
  #[must_use]
  pub fn executor(&self) -> ArcShared<DispatchExecutorRunner<StdToolbox>> {
    self.inner.executor()
  }

  /// Builds a dispatcher using the configured scheduler.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError::InvalidMailboxConfig`] if the mailbox configuration is incompatible
  /// with the executor (e.g., using Block strategy with a non-blocking executor).
  pub fn build_dispatcher(&self, mailbox: ArcShared<MailboxGeneric<StdToolbox>>) -> Result<Dispatcher, SpawnError> {
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
