use alloc::boxed::Box;

use fraktor_utils_rs::core::sync::ArcShared;

#[cfg(test)]
mod tests;

use crate::core::kernel::{
  actor::spawn::SpawnError,
  dispatch::{
    dispatcher::{
      DispatchExecutor, DispatchExecutorRunner, DispatcherBuilder, DispatcherSettings, DispatcherShared,
      InlineExecutor, ScheduleAdapterShared,
    },
    mailbox::{Mailbox, MailboxOverflowStrategy},
  },
};

/// Internal backend configuration produced by dispatcher providers.
pub struct ConfiguredDispatcherBuilder {
  executor: ArcShared<DispatchExecutorRunner>,
  settings: DispatcherSettings,
}

impl Clone for ConfiguredDispatcherBuilder {
  fn clone(&self) -> Self {
    Self { executor: self.executor.clone(), settings: self.settings.clone() }
  }
}

impl ConfiguredDispatcherBuilder {
  /// Creates a configuration from an executor.
  #[must_use]
  pub fn from_executor(executor: Box<dyn DispatchExecutor>) -> Self {
    Self::from_executor_with_settings(executor, DispatcherSettings::default())
  }

  /// Creates a configuration from an executor and immutable settings snapshot.
  #[must_use]
  pub fn from_executor_with_settings(executor: Box<dyn DispatchExecutor>, settings: DispatcherSettings) -> Self {
    Self { executor: ArcShared::new(DispatchExecutorRunner::new(executor)), settings }
  }

  /// Returns the current executor runner handle.
  #[must_use]
  pub fn executor(&self) -> ArcShared<DispatchExecutorRunner> {
    self.executor.clone()
  }

  /// Returns the configured throughput deadline.
  #[must_use]
  pub const fn throughput_deadline(&self) -> Option<core::time::Duration> {
    self.settings.throughput_deadline()
  }

  /// Returns the configured starvation deadline.
  #[must_use]
  pub const fn starvation_deadline(&self) -> Option<core::time::Duration> {
    self.settings.starvation_deadline()
  }

  /// Overrides the throughput deadline.
  #[must_use]
  pub fn with_throughput_deadline(mut self, deadline: Option<core::time::Duration>) -> Self {
    self.settings = self.settings.with_throughput_deadline(deadline);
    self
  }

  /// Overrides the starvation deadline.
  #[must_use]
  pub fn with_starvation_deadline(mut self, deadline: Option<core::time::Duration>) -> Self {
    self.settings = self.settings.with_starvation_deadline(deadline);
    self
  }

  /// Overrides both throughput and starvation deadlines.
  #[must_use]
  pub fn with_deadlines(
    mut self,
    throughput: Option<core::time::Duration>,
    starvation: Option<core::time::Duration>,
  ) -> Self {
    self.settings = self.settings.with_deadlines(throughput, starvation);
    self
  }

  /// Overrides the scheduler adapter used for creating wakers and pending hooks.
  #[must_use]
  pub fn with_schedule_adapter(mut self, adapter: ScheduleAdapterShared) -> Self {
    self.settings = self.settings.with_schedule_adapter(adapter);
    self
  }

  /// Returns the configured schedule adapter.
  #[must_use]
  pub fn schedule_adapter(&self) -> ScheduleAdapterShared {
    self.settings.schedule_adapter()
  }

  /// Returns the immutable settings snapshot.
  #[must_use]
  pub const fn settings(&self) -> &DispatcherSettings {
    &self.settings
  }

  fn build(&self, mailbox: ArcShared<Mailbox>) -> Result<DispatcherShared, SpawnError> {
    let schedule_adapter = self.settings.schedule_adapter();
    let throughput_deadline = self.settings.throughput_deadline();
    let starvation_deadline = self.settings.starvation_deadline();

    // Validate mailbox configuration against executor capabilities
    let policy = mailbox.policy();
    if policy.overflow() == MailboxOverflowStrategy::Block && !self.executor.supports_blocking() {
      return Err(SpawnError::invalid_mailbox_config(
        "MailboxOverflowStrategy::Block requires an executor that supports blocking operations (e.g., \
         TokioExecutor, ThreadedExecutor). InlineExecutor does not support blocking.",
      ));
    }

    Ok(DispatcherShared::with_adapter(
      mailbox,
      self.executor.clone(),
      schedule_adapter,
      throughput_deadline,
      starvation_deadline,
    ))
  }
}

impl DispatcherBuilder for ConfiguredDispatcherBuilder {
  fn settings(&self) -> &DispatcherSettings {
    &self.settings
  }

  /// Builds a dispatcher using the configured executor.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError::InvalidMailboxConfig`] if the mailbox uses
  /// [`MailboxOverflowStrategy::Block`] with an executor that doesn't support blocking operations.
  fn build_dispatcher(&self, mailbox: ArcShared<Mailbox>) -> Result<DispatcherShared, SpawnError> {
    self.build(mailbox)
  }
}

impl Default for ConfiguredDispatcherBuilder {
  fn default() -> Self {
    Self::from_executor(Box::new(InlineExecutor::new()))
  }
}
