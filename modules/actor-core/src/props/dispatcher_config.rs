use core::time::Duration;

use cellactor_utils_core_rs::sync::ArcShared;

#[cfg(test)]
mod tests;

use crate::{
  NoStdToolbox, RuntimeToolbox,
  dispatcher::{DispatchExecutor, DispatcherGeneric, InlineExecutorGeneric},
  mailbox::MailboxGeneric,
};

/// Dispatcher configuration attached to [`Props`](super::Props).
pub struct DispatcherConfigGeneric<TB: RuntimeToolbox + 'static> {
  executor:            ArcShared<dyn DispatchExecutor<TB>>,
  throughput_deadline: Option<Duration>,
  starvation_deadline: Option<Duration>,
}

/// Type alias for [DispatcherConfigGeneric] with the default [NoStdToolbox].
pub type DispatcherConfig = DispatcherConfigGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> Clone for DispatcherConfigGeneric<TB> {
  fn clone(&self) -> Self {
    Self {
      executor:            self.executor.clone(),
      throughput_deadline: self.throughput_deadline,
      starvation_deadline: self.starvation_deadline,
    }
  }
}

impl<TB: RuntimeToolbox + 'static> DispatcherConfigGeneric<TB> {
  /// Creates a configuration from an executor.
  #[must_use]
  pub fn from_executor(executor: ArcShared<dyn DispatchExecutor<TB>>) -> Self {
    Self { executor, throughput_deadline: None, starvation_deadline: None }
  }

  /// Returns the current executor handle.
  #[must_use]
  pub fn executor(&self) -> ArcShared<dyn DispatchExecutor<TB>> {
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

  /// Builds a dispatcher using the configured executor.
  #[must_use]
  pub fn build_dispatcher(&self, mailbox: ArcShared<MailboxGeneric<TB>>) -> DispatcherGeneric<TB> {
    DispatcherGeneric::with_executor(mailbox, self.executor(), self.throughput_deadline, self.starvation_deadline)
  }
}

impl<TB: RuntimeToolbox + 'static> Default for DispatcherConfigGeneric<TB> {
  fn default() -> Self {
    Self::from_executor(ArcShared::new(InlineExecutorGeneric::<TB>::new()))
  }
}
