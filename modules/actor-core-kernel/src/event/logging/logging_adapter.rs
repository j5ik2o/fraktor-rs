//! Classic logging adapter backed by the runtime event stream.

#[cfg(test)]
mod tests;

use alloc::{borrow::ToOwned, collections::BTreeMap, string::String};

use crate::{
  actor::{ActorContext, Pid},
  event::logging::{ActorLogMarker, LogEvent, LogLevel},
  system::ActorSystem,
};

/// Thin classic logging adapter that emits runtime
/// [`crate::event::logging::LogEvent`]s.
#[derive(Clone)]
pub struct LoggingAdapter {
  system:      ActorSystem,
  origin:      Option<Pid>,
  logger_name: Option<String>,
  marker:      Option<ActorLogMarker>,
  mdc:         BTreeMap<String, String>,
}

impl LoggingAdapter {
  /// Creates a new adapter for the supplied origin actor and logger name.
  #[must_use]
  pub const fn new(system: ActorSystem, origin: Option<Pid>, logger_name: Option<String>) -> Self {
    Self { system, origin, logger_name, marker: None, mdc: BTreeMap::new() }
  }

  /// Creates a new adapter bound to the provided classic actor context.
  #[must_use]
  pub fn from_context(context: &ActorContext<'_>) -> Self {
    Self::new(context.system().clone(), Some(context.pid()), context.logger_name().map(ToOwned::to_owned))
  }

  /// Overrides the logger name used for future log events.
  pub fn set_logger_name(&mut self, logger_name: impl Into<String>) {
    self.logger_name = Some(logger_name.into());
  }

  /// Replaces the active marker.
  pub fn set_marker(&mut self, marker: ActorLogMarker) {
    self.marker = Some(marker);
  }

  /// Clears the active marker.
  pub fn clear_marker(&mut self) {
    self.marker = None;
  }

  /// Inserts a diagnostic MDC entry.
  pub fn insert_mdc(&mut self, key: impl Into<String>, value: impl Into<String>) {
    self.mdc.insert(key.into(), value.into());
  }

  /// Clears all diagnostic MDC entries.
  pub fn clear_mdc(&mut self) {
    self.mdc.clear();
  }

  /// Emits a trace-level log event.
  pub fn trace(&self, message: impl Into<String>) {
    self.log(LogLevel::Trace, message);
  }

  /// Emits a debug-level log event.
  pub fn debug(&self, message: impl Into<String>) {
    self.log(LogLevel::Debug, message);
  }

  /// Emits an info-level log event.
  pub fn info(&self, message: impl Into<String>) {
    self.log(LogLevel::Info, message);
  }

  /// Emits a warn-level log event.
  pub fn warn(&self, message: impl Into<String>) {
    self.log(LogLevel::Warn, message);
  }

  /// Emits an error-level log event.
  pub fn error(&self, message: impl Into<String>) {
    self.log(LogLevel::Error, message);
  }

  /// Emits a log event using the provided severity.
  pub fn log(&self, level: LogLevel, message: impl Into<String>) {
    let state = self.system.state();
    let mut event = LogEvent::new(level, message.into(), state.monotonic_now(), self.origin, self.logger_name.clone());

    if let Some(marker) = &self.marker {
      event = event.with_marker(marker.name().to_owned(), marker.properties().clone());
    }
    if !self.mdc.is_empty() {
      event = event.with_mdc(self.mdc.clone());
    }

    state.publish_log_event(event);
  }
}
