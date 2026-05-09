//! Classic actor logging facade with lightweight MDC and marker support.

#[cfg(test)]
mod tests;

use alloc::string::String;

use crate::{
  actor::ActorContext,
  event::logging::{ActorLogMarker, LoggingAdapter},
};

/// Provides a context-bound logging adapter with MDC and marker helpers.
#[derive(Clone)]
pub struct DiagnosticActorLogging {
  adapter: LoggingAdapter,
}

impl DiagnosticActorLogging {
  /// Creates a new diagnostic logging facade for the provided actor context.
  #[must_use]
  pub fn new(context: &ActorContext<'_>) -> Self {
    Self { adapter: LoggingAdapter::from_context(context) }
  }

  /// Returns the diagnostic logging adapter.
  #[must_use]
  pub const fn log(&mut self) -> &mut LoggingAdapter {
    &mut self.adapter
  }

  /// Replaces the active marker metadata.
  pub fn set_marker(&mut self, marker: ActorLogMarker) {
    self.adapter.set_marker(marker);
  }

  /// Clears the active marker metadata.
  pub fn clear_marker(&mut self) {
    self.adapter.clear_marker();
  }

  /// Adds an MDC entry to future log messages.
  pub fn insert_mdc(&mut self, key: impl Into<String>, value: impl Into<String>) {
    self.adapter.insert_mdc(key, value);
  }

  /// Clears all MDC entries.
  pub fn clear_mdc(&mut self) {
    self.adapter.clear_mdc();
  }

  /// Consumes the facade and returns the underlying adapter.
  #[must_use]
  pub fn into_log(self) -> LoggingAdapter {
    self.adapter
  }
}
