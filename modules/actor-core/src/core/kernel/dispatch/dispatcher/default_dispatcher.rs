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
use crate::core::kernel::system::lock_provider::{ActorLockProvider, BuiltinSpinLockProvider};

/// Generic dispatcher that shares its executor across multiple actors.
pub struct DefaultDispatcher {
  core:           DispatcherCore,
  _lock_provider: ArcShared<dyn ActorLockProvider>,
}

impl DefaultDispatcher {
  /// Constructs a new `DefaultDispatcher` with the given settings and executor.
  #[must_use]
  pub fn new(settings: &DispatcherSettings, executor: ExecutorShared) -> Self {
    let lock_provider: ArcShared<dyn ActorLockProvider> = ArcShared::new(BuiltinSpinLockProvider::new());
    Self::new_with_provider(settings, executor, lock_provider)
  }

  /// Constructs a new dispatcher with an explicit actor lock provider.
  #[must_use]
  pub fn new_with_provider(
    settings: &DispatcherSettings,
    executor: ExecutorShared,
    lock_provider: ArcShared<dyn ActorLockProvider>,
  ) -> Self {
    Self { core: DispatcherCore::new(settings, executor), _lock_provider: lock_provider }
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
