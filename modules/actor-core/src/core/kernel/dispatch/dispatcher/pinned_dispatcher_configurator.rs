//! Configurator for [`PinnedDispatcher`](super::PinnedDispatcher).
//!
//! Each call to `dispatcher()` constructs a brand-new dispatcher and executor,
//! matching Pekko's `PinnedDispatcherConfigurator` behaviour. The thread name
//! prefix is captured at construction time.

use alloc::{boxed::Box, string::String};

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::{
  dispatcher_settings::DispatcherSettings, executor_factory::ExecutorFactory,
  message_dispatcher_configurator::MessageDispatcherConfigurator, message_dispatcher_shared::MessageDispatcherShared,
  pinned_dispatcher::PinnedDispatcher,
};
use crate::core::kernel::system::lock_provider::{ActorLockProvider, BuiltinSpinLockProvider};

/// Configurator that produces a fresh [`PinnedDispatcher`] per call.
pub struct PinnedDispatcherConfigurator {
  settings:           DispatcherSettings,
  executor_factory:   ArcShared<Box<dyn ExecutorFactory>>,
  lock_provider:      ArcShared<dyn ActorLockProvider>,
  thread_name_prefix: String,
}

impl PinnedDispatcherConfigurator {
  /// Builds a new pinned configurator.
  #[must_use]
  pub fn new(
    settings: DispatcherSettings,
    executor_factory: ArcShared<Box<dyn ExecutorFactory>>,
    thread_name_prefix: impl Into<String>,
  ) -> Self {
    let lock_provider: ArcShared<dyn ActorLockProvider> = ArcShared::new(BuiltinSpinLockProvider::new());
    Self::new_with_provider(settings, executor_factory, &lock_provider, thread_name_prefix)
  }

  /// Builds a new pinned configurator with an explicit lock provider.
  #[must_use]
  pub fn new_with_provider(
    settings: DispatcherSettings,
    executor_factory: ArcShared<Box<dyn ExecutorFactory>>,
    lock_provider: &ArcShared<dyn ActorLockProvider>,
    thread_name_prefix: impl Into<String>,
  ) -> Self {
    Self {
      settings,
      executor_factory,
      lock_provider: lock_provider.clone(),
      thread_name_prefix: thread_name_prefix.into(),
    }
  }

  /// Returns the thread name prefix configured for new dispatcher instances.
  #[must_use]
  pub fn thread_name_prefix(&self) -> &str {
    &self.thread_name_prefix
  }
}

impl MessageDispatcherConfigurator for PinnedDispatcherConfigurator {
  fn dispatcher(&self) -> MessageDispatcherShared {
    let executor = self.executor_factory.create(self.settings.id());
    let dispatcher = PinnedDispatcher::new(&self.settings, executor);
    self.lock_provider.create_message_dispatcher_shared(Box::new(dispatcher))
  }
}
