use cellactor_utils_core_rs::sync::ArcShared;

use crate::{
  NoStdToolbox, RuntimeToolbox,
  mailbox::Mailbox,
  system::dispatcher::{DispatchExecutor, Dispatcher, InlineExecutor},
};

/// Dispatcher configuration attached to [`Props`](super::Props).
pub struct DispatcherConfig<TB: RuntimeToolbox + 'static = NoStdToolbox> {
  executor: ArcShared<dyn DispatchExecutor<TB>>,
}

impl<TB: RuntimeToolbox + 'static> Clone for DispatcherConfig<TB> {
  fn clone(&self) -> Self {
    Self { executor: self.executor.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> DispatcherConfig<TB> {
  /// Creates a configuration from an executor.
  #[must_use]
  pub fn from_executor(executor: ArcShared<dyn DispatchExecutor<TB>>) -> Self {
    Self { executor }
  }

  /// Returns the current executor handle.
  #[must_use]
  pub fn executor(&self) -> ArcShared<dyn DispatchExecutor<TB>> {
    self.executor.clone()
  }

  /// Builds a dispatcher using the configured executor.
  #[must_use]
  pub fn build_dispatcher(&self, mailbox: ArcShared<Mailbox<TB>>) -> Dispatcher<TB> {
    Dispatcher::new(mailbox, self.executor())
  }
}

impl<TB: RuntimeToolbox + 'static> Default for DispatcherConfig<TB> {
  fn default() -> Self {
    Self::from_executor(ArcShared::new(InlineExecutor::<TB>::new()))
  }
}
