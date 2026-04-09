//! Default concrete `MessageDispatcher` for shared 1:N actor execution.
//!
//! `DefaultDispatcher` carries no behaviour beyond the trait defaults: it
//! delegates lifecycle and dispatch hooks to `MessageDispatcher` and
//! `DispatcherCore`. The Pekko equivalent is `org.apache.pekko.dispatch.Dispatcher`.

#[cfg(test)]
mod tests;

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::{
  dispatcher_core::DispatcherCore, dispatcher_settings::DispatcherSettings, executor_shared::ExecutorShared,
  message_dispatcher::MessageDispatcher,
};
use crate::core::kernel::system::lock_provider::ActorLockProvider;

/// Generic dispatcher that shares its executor across multiple actors.
pub struct DefaultDispatcher {
  core: DispatcherCore,
}

impl DefaultDispatcher {
  /// Constructs a new `DefaultDispatcher` with the given settings and executor.
  #[must_use]
  pub fn new(settings: &DispatcherSettings, executor: ExecutorShared) -> Self {
    Self { core: DispatcherCore::new(settings, executor) }
  }

  /// Constructs a new dispatcher with an explicit actor lock provider.
  ///
  /// This is a no-op wrapper for API compatibility. The lock provider parameter is ignored.
  #[must_use]
  pub fn new_with_provider(
    settings: &DispatcherSettings,
    executor: ExecutorShared,
    _lock_provider: ArcShared<dyn ActorLockProvider>,
  ) -> Self {
    Self::new(settings, executor)
  }
}

impl MessageDispatcher for DefaultDispatcher {
  fn core(&self) -> &DispatcherCore {
    &self.core
  }

  fn core_mut(&mut self) -> &mut DispatcherCore {
    &mut self.core
  }
}
