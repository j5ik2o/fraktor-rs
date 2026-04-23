//! Eager configurator for [`DefaultDispatcher`](super::DefaultDispatcher).

use alloc::boxed::Box;

use super::{
  default_dispatcher::DefaultDispatcher, dispatcher_config::DispatcherConfig, executor_shared::ExecutorShared,
  message_dispatcher_factory::MessageDispatcherFactory, message_dispatcher_shared::MessageDispatcherShared,
};

/// Configurator that holds a single eagerly built [`DefaultDispatcher`] handle.
///
/// `dispatcher()` returns a clone of the cached [`MessageDispatcherShared`],
/// matching Pekko's reuse semantics for non-pinned dispatchers.
pub struct DefaultDispatcherFactory {
  shared: MessageDispatcherShared,
}

impl DefaultDispatcherFactory {
  /// Builds a new configurator from the supplied settings and executor.
  #[must_use]
  pub fn new(settings: &DispatcherConfig, executor: ExecutorShared) -> Self {
    let dispatcher = DefaultDispatcher::new(settings, executor);
    Self { shared: MessageDispatcherShared::new(Box::new(dispatcher)) }
  }
}

impl MessageDispatcherFactory for DefaultDispatcherFactory {
  fn dispatcher(&self) -> MessageDispatcherShared {
    self.shared.clone()
  }
}
