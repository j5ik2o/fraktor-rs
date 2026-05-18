//! Default concrete `MessageDispatcher` for shared 1:N actor execution.
//!
//! `DefaultDispatcher` carries no behaviour beyond the trait defaults: it
//! delegates lifecycle and dispatch hooks to `MessageDispatcher` and
//! `DispatcherCore`. The Pekko equivalent is `org.apache.pekko.dispatch.Dispatcher`.

#[cfg(test)]
#[path = "default_dispatcher_test.rs"]
mod tests;

use super::{
  dispatcher_config::DispatcherConfig, dispatcher_core::DispatcherCore, executor_shared::ExecutorShared,
  message_dispatcher::MessageDispatcher,
};

/// Generic dispatcher that shares its executor across multiple actors.
pub struct DefaultDispatcher {
  core: DispatcherCore,
}

impl DefaultDispatcher {
  /// Constructs a new `DefaultDispatcher` with the given settings and executor.
  #[must_use]
  pub fn new(settings: &DispatcherConfig, executor: ExecutorShared) -> Self {
    Self { core: DispatcherCore::new(settings, executor) }
  }
}

impl MessageDispatcher for DefaultDispatcher {
  fn core(&self) -> &DispatcherCore {
    &self.core
  }

  fn core_mut(&mut self) -> &mut DispatcherCore {
    &mut self.core
  }
}
