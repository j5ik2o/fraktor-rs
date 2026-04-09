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
use crate::core::kernel::runtime_lock_provider::{ActorRuntimeLockProvider, BuiltinSpinRuntimeLockProvider};

/// Configurator that produces a fresh [`PinnedDispatcher`] per call.
pub struct PinnedDispatcherConfigurator {
  settings:              DispatcherSettings,
  executor_factory:      ArcShared<Box<dyn ExecutorFactory>>,
  runtime_lock_provider: ArcShared<dyn ActorRuntimeLockProvider>,
  thread_name_prefix:    String,
}

impl PinnedDispatcherConfigurator {
  /// Builds a new pinned configurator.
  #[must_use]
  pub fn new(
    settings: DispatcherSettings,
    executor_factory: ArcShared<Box<dyn ExecutorFactory>>,
    thread_name_prefix: impl Into<String>,
  ) -> Self {
    Self::new_with_provider(settings, executor_factory, BuiltinSpinRuntimeLockProvider::shared(), thread_name_prefix)
  }

  /// Builds a new pinned configurator with the supplied runtime lock provider.
  #[must_use]
  pub fn new_with_provider(
    settings: DispatcherSettings,
    executor_factory: ArcShared<Box<dyn ExecutorFactory>>,
    provider: ArcShared<dyn ActorRuntimeLockProvider>,
    thread_name_prefix: impl Into<String>,
  ) -> Self {
    Self { settings, executor_factory, runtime_lock_provider: provider, thread_name_prefix: thread_name_prefix.into() }
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
    let dispatcher = PinnedDispatcher::new_with_provider(&self.settings, executor, self.runtime_lock_provider.clone());
    MessageDispatcherShared::new_with_provider(dispatcher, self.runtime_lock_provider.clone())
  }
}
