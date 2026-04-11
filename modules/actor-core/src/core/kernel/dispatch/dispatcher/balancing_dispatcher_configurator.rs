//! Eager configurator for [`BalancingDispatcher`](super::BalancingDispatcher).

use alloc::boxed::Box;

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::{
  balancing_dispatcher::BalancingDispatcher, dispatcher_settings::DispatcherSettings, executor_shared::ExecutorShared,
  message_dispatcher_configurator::MessageDispatcherConfigurator, message_dispatcher_shared::MessageDispatcherShared,
  message_dispatcher_shared_factory::MessageDispatcherSharedFactory,
  shared_message_queue_factory::SharedMessageQueueFactory,
};
use crate::core::kernel::system::shared_factory::MailboxSharedSetFactory;

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
  pub fn new(
    settings: &DispatcherSettings,
    executor: ExecutorShared,
    message_dispatcher_shared_factory: &ArcShared<dyn MessageDispatcherSharedFactory>,
    shared_message_queue_factory: &ArcShared<dyn SharedMessageQueueFactory>,
    mailbox_shared_set_factory: &ArcShared<dyn MailboxSharedSetFactory>,
  ) -> Self {
    let dispatcher =
      BalancingDispatcher::new(settings, executor, shared_message_queue_factory, mailbox_shared_set_factory);
    Self { shared: message_dispatcher_shared_factory.create(Box::new(dispatcher)) }
  }
}

impl MessageDispatcherConfigurator for BalancingDispatcherConfigurator {
  fn dispatcher(&self) -> MessageDispatcherShared {
    self.shared.clone()
  }
}
