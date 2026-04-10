//! Error type for status-aware ask responses.

#[cfg(test)]
mod tests;

use alloc::string::String;
use core::fmt::{Display, Formatter, Result as FmtResult};

/// Error returned by a status-aware ask when the responder reports failure.
#[derive(Clone, Debug)]
pub struct StatusReplyError {
  message: String,
}

impl StatusReplyError {
  /// Creates a new status reply error with the given message.
  #[must_use]
  pub fn new(message: impl Into<String>) -> Self {
    Self { message: message.into() }
  }

  /// Returns the error message.
  #[must_use]
  pub fn message(&self) -> &str {
    &self.message
  }
}

impl Display for StatusReplyError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    write!(f, "{}", self.message)
  }
}
