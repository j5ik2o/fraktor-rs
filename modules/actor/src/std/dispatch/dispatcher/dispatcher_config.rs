extern crate alloc;
use alloc::boxed::Box;

use fraktor_utils_rs::core::sync::ArcShared;
#[cfg(feature = "tokio-executor")]
use tokio::runtime::Handle;

use super::StdScheduleAdapter;
#[cfg(feature = "tokio-executor")]
use super::dispatch_executor::TokioExecutor;
use crate::core::{
  dispatch::{
    dispatcher::{
      DispatchExecutor, DispatchExecutorRunner, DispatcherConfig as CoreDispatcherConfig, DispatcherShared,
      ScheduleAdapterShared,
    },
    mailbox::Mailbox,
  },
  spawn::SpawnError,
};

#[cfg(all(test, feature = "tokio-executor"))]
mod tests;

/// Dispatcher configuration for the standard runtime.
#[derive(Clone, Default)]
pub struct DispatcherConfig {
  inner: CoreDispatcherConfig,
}

impl DispatcherConfig {
  /// Creates a configuration from a scheduler implementation.
  #[must_use]
  pub fn from_executor(executor: Box<dyn DispatchExecutor>) -> Self {
    let schedule_adapter = ScheduleAdapterShared::new(Box::new(StdScheduleAdapter::default()));
    let inner = CoreDispatcherConfig::from_executor(executor).with_schedule_adapter(schedule_adapter);
    Self { inner }
  }

  /// Creates a default configuration from the current Tokio runtime handle.
  ///
  /// Returns `None` when called outside a Tokio runtime context.
  #[cfg(feature = "tokio-executor")]
  #[must_use]
  pub fn try_default_config() -> Option<Self> {
    Handle::try_current().ok().map(|handle| Self::from_executor(Box::new(TokioExecutor::new(handle))))
  }

  /// Creates a default configuration from the current Tokio runtime handle.
  ///
  /// # Panics
  ///
  /// Panics when called outside a Tokio runtime context.
  #[cfg(feature = "tokio-executor")]
  #[must_use]
  pub fn default_config() -> Self {
    Self::try_default_config().expect("Tokio runtime handle unavailable")
  }

  /// Creates a configuration from the current Tokio runtime handle.
  ///
  /// # Panics
  ///
  /// Panics when called outside a Tokio runtime context.
  #[cfg(feature = "tokio-executor")]
  #[must_use]
  #[deprecated(since = "0.3.0", note = "use DispatcherConfig::default_config() instead; will be removed in v0.4.0")]
  pub fn tokio_auto() -> Self {
    Self::default_config()
  }

  /// Returns the configured scheduler runner.
  ///
  /// The returned [`DispatchExecutorRunner`] implements [`DispatchExecutor`] and can be used
  /// to submit dispatchers for execution.
  #[must_use]
  pub fn executor(&self) -> ArcShared<DispatchExecutorRunner> {
    self.inner.executor()
  }

  /// Builds a dispatcher using the configured scheduler.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError::InvalidMailboxConfig`] if the mailbox configuration is incompatible
  /// with the executor (e.g., using Block strategy with a non-blocking executor).
  pub fn build_dispatcher(&self, mailbox: ArcShared<Mailbox>) -> Result<DispatcherShared, SpawnError> {
    self.inner.build_dispatcher(mailbox)
  }

  /// Borrows the underlying core configuration.
  #[must_use]
  pub const fn as_core(&self) -> &CoreDispatcherConfig {
    &self.inner
  }

  /// Consumes the wrapper and returns the core configuration.
  #[must_use]
  pub fn into_core(self) -> CoreDispatcherConfig {
    self.inner
  }

  /// Wraps an existing core configuration.
  #[must_use]
  pub const fn from_core(inner: CoreDispatcherConfig) -> Self {
    Self { inner }
  }
}
