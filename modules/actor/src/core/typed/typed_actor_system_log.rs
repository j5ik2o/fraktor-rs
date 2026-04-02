//! System-level log handle for [`TypedActorSystem`].

use alloc::string::String;

use crate::core::kernel::{event::logging::LogLevel, system::ActorSystem};

/// Thin log handle returned by [`TypedActorSystem::log`].
#[derive(Clone)]
pub struct TypedActorSystemLog {
  system: ActorSystem,
}

impl TypedActorSystemLog {
  /// Creates a new log handle bound to the provided actor system.
  #[must_use]
  pub(crate) const fn new(system: ActorSystem) -> Self {
    Self { system }
  }

  /// Emits a log event through the actor system event stream.
  pub fn emit(&self, level: LogLevel, message: impl Into<String>) {
    self.system.emit_log(level, message, None, None);
  }
}
