//! Eager configurator for [`DefaultDispatcher`](super::DefaultDispatcher).

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::{
  default_dispatcher::DefaultDispatcher, dispatcher_settings::DispatcherSettings, executor_shared::ExecutorShared,
  message_dispatcher_configurator::MessageDispatcherConfigurator, message_dispatcher_shared::MessageDispatcherShared,
};
use crate::core::kernel::runtime_lock_provider::{ActorRuntimeLockProvider, BuiltinSpinRuntimeLockProvider};

/// Configurator that holds a single eagerly built [`DefaultDispatcher`] handle.
///
/// `dispatcher()` returns a clone of the cached [`MessageDispatcherShared`],
/// matching Pekko's reuse semantics for non-pinned dispatchers.
pub struct DefaultDispatcherConfigurator {
  shared: MessageDispatcherShared,
}

impl DefaultDispatcherConfigurator {
  /// Builds a new configurator from the supplied settings and executor.
  #[must_use]
  pub fn new(settings: &DispatcherSettings, executor: ExecutorShared) -> Self {
    Self::new_with_provider(settings, executor, BuiltinSpinRuntimeLockProvider::shared())
  }

  /// Builds a new configurator from the supplied settings, executor, and runtime lock provider.
  #[must_use]
  pub fn new_with_provider(
    settings: &DispatcherSettings,
    executor: ExecutorShared,
    provider: ArcShared<dyn ActorRuntimeLockProvider>,
  ) -> Self {
    let dispatcher = DefaultDispatcher::new_with_provider(settings, executor, provider.clone());
    Self { shared: MessageDispatcherShared::new_with_provider(dispatcher, provider) }
  }
}

impl MessageDispatcherConfigurator for DefaultDispatcherConfigurator {
  fn dispatcher(&self) -> MessageDispatcherShared {
    self.shared.clone()
  }
}
