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
use crate::core::kernel::runtime_lock_provider::{ActorRuntimeLockProvider, BuiltinSpinRuntimeLockProvider};

/// Generic dispatcher that shares its executor across multiple actors.
pub struct DefaultDispatcher {
  core:                  DispatcherCore,
  runtime_lock_provider: ArcShared<dyn ActorRuntimeLockProvider>,
}

impl DefaultDispatcher {
  /// Constructs a new `DefaultDispatcher` with the given settings and executor.
  #[must_use]
  pub fn new(settings: &DispatcherSettings, executor: ExecutorShared) -> Self {
    Self::new_with_provider(settings, executor, BuiltinSpinRuntimeLockProvider::shared())
  }

  /// Constructs a new `DefaultDispatcher` with the given settings, executor, and runtime lock
  /// provider.
  #[must_use]
  pub fn new_with_provider(
    settings: &DispatcherSettings,
    executor: ExecutorShared,
    provider: ArcShared<dyn ActorRuntimeLockProvider>,
  ) -> Self {
    Self { core: DispatcherCore::new(settings, executor), runtime_lock_provider: provider }
  }

  /// Returns the bound runtime lock provider.
  #[must_use]
  pub fn runtime_lock_provider(&self) -> ArcShared<dyn ActorRuntimeLockProvider> {
    self.runtime_lock_provider.clone()
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
