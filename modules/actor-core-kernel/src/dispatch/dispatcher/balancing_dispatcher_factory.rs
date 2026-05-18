//! Eager configurator for [`BalancingDispatcher`](super::BalancingDispatcher).

use alloc::boxed::Box;

use super::{
  balancing_dispatcher::BalancingDispatcher, dispatcher_config::DispatcherConfig, executor_shared::ExecutorShared,
  message_dispatcher_factory::MessageDispatcherFactory, message_dispatcher_shared::MessageDispatcherShared,
  shared_message_queue::SharedMessageQueue,
};

/// Configurator that holds a single eagerly built [`BalancingDispatcher`] handle.
///
/// Like [`DefaultDispatcherFactory`](super::DefaultDispatcherFactory),
/// `dispatcher()` returns a clone of the cached handle so that all actors
/// share the same dispatcher (and thus the same shared message queue).
pub struct BalancingDispatcherFactory {
  shared: MessageDispatcherShared,
}

impl BalancingDispatcherFactory {
  /// Builds a new configurator from the supplied settings and executor.
  #[must_use]
  pub fn new(settings: &DispatcherConfig, executor: ExecutorShared, shared_queue: SharedMessageQueue) -> Self {
    let dispatcher = BalancingDispatcher::new(settings, executor, shared_queue);
    Self { shared: MessageDispatcherShared::new(Box::new(dispatcher)) }
  }
}

impl MessageDispatcherFactory for BalancingDispatcherFactory {
  fn dispatcher(&self) -> MessageDispatcherShared {
    self.shared.clone()
  }
}
