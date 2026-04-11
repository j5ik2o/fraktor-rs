//! Eager configurator for [`DefaultDispatcher`](super::DefaultDispatcher).

use alloc::boxed::Box;

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::{
  default_dispatcher::DefaultDispatcher, dispatcher_settings::DispatcherSettings, executor_shared::ExecutorShared,
  message_dispatcher::MessageDispatcher, message_dispatcher_configurator::MessageDispatcherConfigurator,
  message_dispatcher_shared::MessageDispatcherShared,
};
use crate::core::kernel::system::lock_provider::ActorLockProvider;

/// Configurator that holds a single eagerly built [`DefaultDispatcher`] handle.
///
/// `dispatcher()` returns a clone of the cached [`MessageDispatcherShared`],
/// matching Pekko's reuse semantics for non-pinned dispatchers.
pub struct DefaultDispatcherConfigurator {
  shared: MessageDispatcherShared,
}

impl DefaultDispatcherConfigurator {
  /// Builds a new configurator from the supplied settings and executor.
  ///
  /// The cached [`MessageDispatcherShared`] is wrapped using the workspace's
  /// compile-time selected default lock driver. Use
  /// [`Self::new_with_provider`] only when a runtime [`ActorLockProvider`]
  /// override is in effect (e.g. tests installing `DebugActorLockProvider`).
  #[must_use]
  pub fn new(settings: &DispatcherSettings, executor: ExecutorShared) -> Self {
    let dispatcher = DefaultDispatcher::new(settings, executor);
    Self { shared: MessageDispatcherShared::new_with_builtin_lock(dispatcher) }
  }

  /// Builds a configurator that binds the supplied actor lock provider.
  ///
  /// Used by `Dispatchers::*_with_provider` paths when a runtime
  /// [`ActorLockProvider`] override has been installed at the actor system
  /// boundary.
  #[must_use]
  pub fn new_with_provider(
    settings: &DispatcherSettings,
    executor: ExecutorShared,
    provider: &ArcShared<dyn ActorLockProvider>,
  ) -> Self {
    let dispatcher: Box<dyn MessageDispatcher> = Box::new(DefaultDispatcher::new(settings, executor));
    Self { shared: provider.create_message_dispatcher_shared(dispatcher) }
  }
}

impl MessageDispatcherConfigurator for DefaultDispatcherConfigurator {
  fn dispatcher(&self) -> MessageDispatcherShared {
    self.shared.clone()
  }
}
