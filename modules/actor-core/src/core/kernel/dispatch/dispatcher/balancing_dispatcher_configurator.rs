//! Eager configurator for [`BalancingDispatcher`](super::BalancingDispatcher).

use alloc::boxed::Box;

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::{
  balancing_dispatcher::BalancingDispatcher, dispatcher_settings::DispatcherSettings, executor_shared::ExecutorShared,
  message_dispatcher_configurator::MessageDispatcherConfigurator, message_dispatcher_shared::MessageDispatcherShared,
};
use crate::core::kernel::system::lock_provider::{ActorLockProvider, BuiltinSpinLockProvider};

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
    let provider: ArcShared<dyn ActorLockProvider> = ArcShared::new(BuiltinSpinLockProvider::new());
    Self::new_with_provider(settings, executor, &provider)
  }

  /// Builds a configurator that binds the supplied actor lock provider.
  #[must_use]
  pub fn new_with_provider(
    settings: &DispatcherSettings,
    executor: ExecutorShared,
    provider: &ArcShared<dyn ActorLockProvider>,
  ) -> Self {
    let dispatcher = BalancingDispatcher::new_with_provider(settings, executor, provider.clone());
    Self { shared: provider.create_message_dispatcher_shared(Box::new(dispatcher)) }
  }
}

impl MessageDispatcherConfigurator for BalancingDispatcherConfigurator {
  fn dispatcher(&self) -> MessageDispatcherShared {
    self.shared.clone()
  }
}
