//! Eager configurator for [`DefaultDispatcher`](super::DefaultDispatcher).

use alloc::boxed::Box;

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::{
  default_dispatcher::DefaultDispatcher, dispatcher_settings::DispatcherSettings, executor_shared::ExecutorShared,
  message_dispatcher_configurator::MessageDispatcherConfigurator, message_dispatcher_shared::MessageDispatcherShared,
  message_dispatcher_shared_factory::MessageDispatcherSharedFactory,
};

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
  pub fn new(
    settings: &DispatcherSettings,
    executor: ExecutorShared,
    factory: &ArcShared<dyn MessageDispatcherSharedFactory>,
  ) -> Self {
    let dispatcher = DefaultDispatcher::new(settings, executor);
    Self { shared: factory.create_message_dispatcher_shared(Box::new(dispatcher)) }
  }
}

impl MessageDispatcherConfigurator for DefaultDispatcherConfigurator {
  fn dispatcher(&self) -> MessageDispatcherShared {
    self.shared.clone()
  }
}
