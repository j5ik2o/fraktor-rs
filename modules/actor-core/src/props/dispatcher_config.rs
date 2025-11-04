use cellactor_utils_core_rs::sync::ArcShared;

use crate::{
  NoStdToolbox, RuntimeToolbox,
  dispatcher::{DispatchExecutor, DispatcherGeneric, InlineExecutor},
  mailbox::MailboxGeneric,
};

/// Dispatcher configuration attached to [`Props`](super::Props).
pub struct DispatcherConfigGeneric<TB: RuntimeToolbox + 'static> {
  executor: ArcShared<dyn DispatchExecutor<TB>>,
}

impl<TB: RuntimeToolbox + 'static> Clone for DispatcherConfigGeneric<TB> {
  fn clone(&self) -> Self {
    Self { executor: self.executor.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> DispatcherConfigGeneric<TB> {
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
  pub fn build_dispatcher(&self, mailbox: ArcShared<MailboxGeneric<TB>>) -> DispatcherGeneric<TB> {
    DispatcherGeneric::new(mailbox, self.executor())
  }
}

impl<TB: RuntimeToolbox + 'static> Default for DispatcherConfigGeneric<TB> {
  fn default() -> Self {
    Self::from_executor(ArcShared::new(InlineExecutor::<TB>::new()))
  }
}

/// Type alias for `DispatcherConfigGeneric` with the default `NoStdToolbox`.
pub type DispatcherConfig = DispatcherConfigGeneric<NoStdToolbox>;
