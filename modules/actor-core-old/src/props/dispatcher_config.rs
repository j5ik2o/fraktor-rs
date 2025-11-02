use cellactor_utils_core_rs::sync::ArcShared;

use crate::{
  dispatcher::{DispatchExecutor, Dispatcher, InlineExecutor},
  mailbox::Mailbox,
};

/// Dispatcher configuration attached to [`Props`](super::Props).
#[derive(Clone)]
pub struct DispatcherConfig {
  executor: ArcShared<dyn DispatchExecutor>,
}

impl DispatcherConfig {
  #[must_use]
  /// Creates a configuration from an executor.
  pub fn from_executor(executor: ArcShared<dyn DispatchExecutor>) -> Self {
    Self { executor }
  }

  #[must_use]
  /// Returns the current executor handle.
  pub fn executor(&self) -> ArcShared<dyn DispatchExecutor> {
    self.executor.clone()
  }

  #[must_use]
  /// Builds a dispatcher using the configured executor.
  pub fn build_dispatcher(&self, mailbox: ArcShared<Mailbox>) -> Dispatcher {
    Dispatcher::new(mailbox, self.executor())
  }
}

impl Default for DispatcherConfig {
  fn default() -> Self {
    Self::from_executor(ArcShared::new(InlineExecutor::new()))
  }
}
