//! Eager configurator for [`BalancingDispatcher`](super::BalancingDispatcher).

use alloc::boxed::Box;

use super::{
  balancing_dispatcher::BalancingDispatcher, dispatcher_config::DispatcherConfig, executor_shared::ExecutorShared,
  message_dispatcher_factory::MessageDispatcherFactory, message_dispatcher_shared::MessageDispatcherShared,
  shared_message_queue::SharedMessageQueue,
};
use crate::{actor::props::MailboxRequirement, dispatch::mailbox::MailboxFactory};

/// Configurator that holds a single eagerly built [`BalancingDispatcher`] handle.
///
/// Like [`DefaultDispatcherFactory`](super::DefaultDispatcherFactory),
/// `dispatcher()` returns a clone of the cached handle so that all actors
/// share the same dispatcher (and thus the same shared message queue).
pub struct BalancingDispatcherFactory {
  shared: MessageDispatcherShared,
}

impl BalancingDispatcherFactory {
  /// Builds a new configurator from the supplied configuration and executor.
  #[must_use]
  pub fn new(config: &DispatcherConfig, executor: ExecutorShared, shared_queue: SharedMessageQueue) -> Self {
    let dispatcher = BalancingDispatcher::new(config, executor, shared_queue);
    Self { shared: MessageDispatcherShared::new(Box::new(dispatcher)) }
  }

  /// Builds a new configurator after validating mailbox compatibility.
  ///
  /// Returns `None` when `mailbox_factory` does not advertise
  /// multiple-consumer queue semantics.
  #[must_use]
  pub fn new_checked(
    config: &DispatcherConfig,
    executor: ExecutorShared,
    shared_queue: SharedMessageQueue,
    mailbox_factory: &dyn MailboxFactory,
  ) -> Option<Self> {
    if Self::is_mailbox_compatible(mailbox_factory) { Some(Self::new(config, executor, shared_queue)) } else { None }
  }

  /// Returns whether `mailbox_factory` satisfies the balancing dispatcher mailbox contract.
  #[must_use]
  pub fn is_mailbox_compatible(mailbox_factory: &dyn MailboxFactory) -> bool {
    mailbox_factory.produced_queue_semantics().satisfies(MailboxRequirement::requires_multiple_consumer())
  }
}

impl MessageDispatcherFactory for BalancingDispatcherFactory {
  fn dispatcher(&self) -> MessageDispatcherShared {
    self.shared.clone()
  }
}
