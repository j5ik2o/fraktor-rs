//! Eager configurator for [`BalancingDispatcher`](super::BalancingDispatcher).

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::{
  balancing_dispatcher::BalancingDispatcher, dispatcher_settings::DispatcherSettings, executor_shared::ExecutorShared,
  message_dispatcher_configurator::MessageDispatcherConfigurator, message_dispatcher_shared::MessageDispatcherShared,
};
use crate::core::kernel::runtime_lock_provider::{ActorRuntimeLockProvider, BuiltinSpinRuntimeLockProvider};

/// Configurator that holds a single eagerly built [`BalancingDispatcher`] handle.
///
/// Like [`DefaultDispatcherConfigurator`](super::DefaultDispatcherConfigurator),
/// `dispatcher()` returns a clone of the cached handle so that all actors
/// share the same dispatcher (and thus the same shared message queue).
pub struct BalancingDispatcherConfigurator {
  shared: MessageDispatcherShared,
}

impl BalancingDispatcherConfigurator {
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
    let dispatcher = BalancingDispatcher::new_with_provider(settings, executor, provider.clone());
    Self { shared: MessageDispatcherShared::new_with_provider(dispatcher, provider) }
  }
}

impl MessageDispatcherConfigurator for BalancingDispatcherConfigurator {
  fn dispatcher(&self) -> MessageDispatcherShared {
    self.shared.clone()
  }
}
