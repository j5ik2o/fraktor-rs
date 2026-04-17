//! System-level log handle for [`TypedActorSystem`].

#[cfg(test)]
mod tests;

use alloc::{fmt::format, string::String};
use core::fmt::Arguments;

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

  /// Returns `true` when the configured filter would accept events of the given `level`.
  ///
  /// Callers using `*_fmt` methods do not need to consult this directly; the
  /// methods already short-circuit when the level is disabled. This accessor is
  /// provided for callers that want to skip building structured log context.
  #[must_use]
  pub fn is_level_enabled(&self, level: LogLevel) -> bool {
    self.system.is_log_level_enabled(level)
  }

  /// Emits a TRACE-level log event by rendering `args` only when the level is enabled.
  ///
  /// Mirrors Pekko's `LoggerOps.trace(template, args...)` lazy contract: the
  /// supplied `Arguments` are never formatted if TRACE is disabled by the
  /// current logging filter.
  pub fn trace_fmt(&self, args: Arguments<'_>) {
    self.emit_fmt(LogLevel::Trace, args);
  }

  /// Emits a DEBUG-level log event with lazy formatting semantics.
  pub fn debug_fmt(&self, args: Arguments<'_>) {
    self.emit_fmt(LogLevel::Debug, args);
  }

  /// Emits an INFO-level log event with lazy formatting semantics.
  pub fn info_fmt(&self, args: Arguments<'_>) {
    self.emit_fmt(LogLevel::Info, args);
  }

  /// Emits a WARN-level log event with lazy formatting semantics.
  pub fn warn_fmt(&self, args: Arguments<'_>) {
    self.emit_fmt(LogLevel::Warn, args);
  }

  /// Emits an ERROR-level log event with lazy formatting semantics.
  pub fn error_fmt(&self, args: Arguments<'_>) {
    self.emit_fmt(LogLevel::Error, args);
  }

  fn emit_fmt(&self, level: LogLevel, args: Arguments<'_>) {
    if !self.system.is_log_level_enabled(level) {
      return;
    }
    self.system.emit_log(level, format(args), None, None);
  }
}
