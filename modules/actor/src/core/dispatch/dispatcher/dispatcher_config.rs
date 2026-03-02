use alloc::boxed::Box;
use core::time::Duration;

use fraktor_utils_rs::core::sync::ArcShared;

#[cfg(test)]
mod tests;

use crate::core::{
  dispatch::{
    dispatcher::{
      DispatchExecutor, DispatchExecutorRunner, DispatcherShared, InlineExecutor, InlineScheduleAdapter,
      ScheduleAdapterShared,
    },
    mailbox::{Mailbox, MailboxOverflowStrategy},
  },
  spawn::SpawnError,
};

/// Dispatcher configuration attached to [`Props`](crate::core::props::Props).
pub struct DispatcherConfig {
  executor:            ArcShared<DispatchExecutorRunner>,
  throughput_deadline: Option<Duration>,
  starvation_deadline: Option<Duration>,
  schedule_adapter:    ScheduleAdapterShared,
}

impl Clone for DispatcherConfig {
  fn clone(&self) -> Self {
    Self {
      executor:            self.executor.clone(),
      throughput_deadline: self.throughput_deadline,
      starvation_deadline: self.starvation_deadline,
      schedule_adapter:    self.schedule_adapter.clone(),
    }
  }
}

impl DispatcherConfig {
  /// Creates a configuration from an executor.
  #[must_use]
  pub fn from_executor(executor: Box<dyn DispatchExecutor>) -> Self {
    Self {
      executor:            ArcShared::new(DispatchExecutorRunner::new(executor)),
      throughput_deadline: None,
      starvation_deadline: None,
      schedule_adapter:    InlineScheduleAdapter::shared(),
    }
  }

  /// Returns the current executor runner handle.
  #[must_use]
  pub fn executor(&self) -> ArcShared<DispatchExecutorRunner> {
    self.executor.clone()
  }

  /// Returns the configured throughput deadline.
  #[must_use]
  pub const fn throughput_deadline(&self) -> Option<Duration> {
    self.throughput_deadline
  }

  /// Returns the configured starvation deadline.
  #[must_use]
  pub const fn starvation_deadline(&self) -> Option<Duration> {
    self.starvation_deadline
  }

  /// Overrides the throughput deadline.
  #[must_use]
  pub const fn with_throughput_deadline(mut self, deadline: Option<Duration>) -> Self {
    self.throughput_deadline = deadline;
    self
  }

  /// Overrides the starvation deadline.
  #[must_use]
  pub const fn with_starvation_deadline(mut self, deadline: Option<Duration>) -> Self {
    self.starvation_deadline = deadline;
    self
  }

  /// Overrides both throughput and starvation deadlines.
  #[must_use]
  pub const fn with_deadlines(mut self, throughput: Option<Duration>, starvation: Option<Duration>) -> Self {
    self.throughput_deadline = throughput;
    self.starvation_deadline = starvation;
    self
  }

  /// Overrides the scheduler adapter used for creating wakers and pending hooks.
  #[must_use]
  pub fn with_schedule_adapter(mut self, adapter: ScheduleAdapterShared) -> Self {
    self.schedule_adapter = adapter;
    self
  }

  /// Returns the configured schedule adapter.
  #[must_use]
  pub fn schedule_adapter(&self) -> ScheduleAdapterShared {
    self.schedule_adapter.clone()
  }

  /// Builds a dispatcher using the configured executor.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError::InvalidMailboxConfig`] if the mailbox uses
  /// [`MailboxOverflowStrategy::Block`] with an executor that doesn't support blocking operations.
  pub fn build_dispatcher(&self, mailbox: ArcShared<Mailbox>) -> Result<DispatcherShared, SpawnError> {
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
      self.schedule_adapter(),
      self.throughput_deadline,
      self.starvation_deadline,
    ))
  }
}

impl Default for DispatcherConfig {
  fn default() -> Self {
    Self::from_executor(Box::new(InlineExecutor::new()))
  }
}
