use cellactor_actor_core_rs::{
  dispatcher::{DispatchExecutor as CoreDispatchExecutor, ScheduleAdapter},
  mailbox::MailboxGeneric,
  props::DispatcherConfigGeneric as CoreDispatcherConfigGeneric,
};
use cellactor_utils_core_rs::sync::ArcShared;
use cellactor_utils_std_rs::runtime_toolbox::StdToolbox;

use super::{CoreDispatchExecutorAdapter, DispatchExecutor, DispatchExecutorAdapter, Dispatcher, StdScheduleAdapter};

/// Dispatcher configuration specialised for `StdToolbox`.
#[derive(Clone, Default)]
pub struct DispatcherConfig {
  inner: CoreDispatcherConfigGeneric<StdToolbox>,
}

impl DispatcherConfig {
  /// Creates a configuration from a scheduler implementation.
  #[must_use]
  pub fn from_executor(executor: ArcShared<dyn DispatchExecutor>) -> Self {
    let executor_adapter: ArcShared<dyn CoreDispatchExecutor<StdToolbox>> =
      ArcShared::new(DispatchExecutorAdapter::new(executor));
    let schedule_adapter: ArcShared<dyn ScheduleAdapter<StdToolbox>> = ArcShared::new(StdScheduleAdapter::default());
    let inner = CoreDispatcherConfigGeneric::from_executor(executor_adapter).with_schedule_adapter(schedule_adapter);
    Self { inner }
  }

  /// Returns the configured scheduler as a standard trait object.
  #[must_use]
  pub fn executor(&self) -> ArcShared<dyn DispatchExecutor> {
    let core_executor = self.inner.executor();
    ArcShared::new(CoreDispatchExecutorAdapter::new(core_executor))
  }

  /// Builds a dispatcher using the configured scheduler.
  #[must_use]
  pub fn build_dispatcher(&self, mailbox: ArcShared<MailboxGeneric<StdToolbox>>) -> Dispatcher {
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
