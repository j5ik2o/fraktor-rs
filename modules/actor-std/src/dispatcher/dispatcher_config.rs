use cellactor_actor_core_rs::{
  dispatcher::DispatchExecutor as CoreDispatchExecutor, mailbox::Mailbox,
  props::DispatcherConfig as CoreDispatcherConfig,
};
use cellactor_utils_core_rs::sync::ArcShared;
use cellactor_utils_std_rs::StdToolbox;

use super::{CoreDispatchExecutorAdapter, DispatchExecutor, DispatchExecutorAdapter, Dispatcher};

/// Dispatcher configuration specialised for `StdToolbox`.
#[derive(Clone)]
pub struct DispatcherConfig {
  inner: CoreDispatcherConfig<StdToolbox>,
}

impl DispatcherConfig {
  /// Creates a configuration from a scheduler implementation.
  #[must_use]
  pub fn from_executor(executor: ArcShared<dyn DispatchExecutor>) -> Self {
    let adapter: ArcShared<dyn CoreDispatchExecutor<StdToolbox>> =
      ArcShared::new(DispatchExecutorAdapter::new(executor));
    Self { inner: CoreDispatcherConfig::from_executor(adapter) }
  }

  /// Returns the configured scheduler as a standard trait object.
  #[must_use]
  pub fn executor(&self) -> ArcShared<dyn DispatchExecutor> {
    let core_executor = self.inner.executor();
    ArcShared::new(CoreDispatchExecutorAdapter::new(core_executor))
  }

  /// Builds a dispatcher using the configured scheduler.
  #[must_use]
  pub fn build_dispatcher(&self, mailbox: ArcShared<Mailbox<StdToolbox>>) -> Dispatcher {
    self.inner.build_dispatcher(mailbox)
  }

  /// Borrows the underlying core configuration.
  #[must_use]
  pub const fn as_core(&self) -> &CoreDispatcherConfig<StdToolbox> {
    &self.inner
  }

  /// Consumes the wrapper and returns the core configuration.
  #[must_use]
  pub fn into_core(self) -> CoreDispatcherConfig<StdToolbox> {
    self.inner
  }

  /// Wraps an existing core configuration.
  #[must_use]
  pub const fn from_core(inner: CoreDispatcherConfig<StdToolbox>) -> Self {
    Self { inner }
  }
}

impl Default for DispatcherConfig {
  fn default() -> Self {
    Self { inner: CoreDispatcherConfig::default() }
  }
}
