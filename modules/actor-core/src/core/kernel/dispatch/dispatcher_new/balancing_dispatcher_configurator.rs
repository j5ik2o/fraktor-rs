//! Eager configurator for [`BalancingDispatcher`](super::BalancingDispatcher).

use super::{
  balancing_dispatcher::BalancingDispatcher, dispatcher_settings::DispatcherSettings, executor_shared::ExecutorShared,
  message_dispatcher_configurator::MessageDispatcherConfigurator, message_dispatcher_shared::MessageDispatcherShared,
};

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
    let dispatcher = BalancingDispatcher::new(settings, executor);
    Self { shared: MessageDispatcherShared::new(dispatcher) }
  }
}

impl MessageDispatcherConfigurator for BalancingDispatcherConfigurator {
  fn dispatcher(&self) -> MessageDispatcherShared {
    self.shared.clone()
  }
}
