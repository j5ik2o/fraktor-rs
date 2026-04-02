//! Receive decorator that logs handled and unhandled messages.

#[cfg(test)]
mod tests;

use alloc::{format, string::String};
use core::fmt::Debug;

use super::logging_adapter::LoggingAdapter;
use crate::core::kernel::{actor::ActorContext, event::logging::LogLevel};

/// Logs receive attempts using a classic actor context.
#[derive(Clone, Debug)]
pub struct LoggingReceive {
  label: Option<String>,
  level: LogLevel,
}

impl LoggingReceive {
  /// Creates a new receive decorator with the provided log level.
  #[must_use]
  pub const fn new(level: LogLevel) -> Self {
    Self { label: None, level }
  }

  /// Creates a debug-level decorator that appends the supplied state label.
  #[must_use]
  pub fn with_label(label: impl Into<String>) -> Self {
    Self { label: Some(label.into()), level: LogLevel::Debug }
  }

  /// Creates a decorator with the supplied state label and log level.
  #[must_use]
  pub fn with_label_and_level(label: impl Into<String>, level: LogLevel) -> Self {
    Self { label: Some(label.into()), level }
  }

  /// Emits a receive log message through the actor context.
  pub fn log<M>(&self, context: &ActorContext<'_>, message: &M, handled: bool)
  where
    M: Debug, {
    let adapter = LoggingAdapter::from_context(context);
    let handled_state = if handled { "handled" } else { "unhandled" };
    let sender =
      context.sender().map(|sender| format!("{:?}", sender.pid())).unwrap_or_else(|| String::from("noSender"));
    let label = self.label.as_ref().map(|label| format!(" in state {label}")).unwrap_or_default();

    adapter.log(self.level, format!("received {handled_state} message {message:?} from {sender}{label}"));
  }
}

impl Default for LoggingReceive {
  fn default() -> Self {
    Self::new(LogLevel::Debug)
  }
}
