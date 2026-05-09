use core::fmt::{Display, Formatter, Result as FmtResult};

/// Error reported by shutdown tasks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TaskRunError {
  message: &'static str,
}

impl TaskRunError {
  /// Creates a new error with the provided message.
  #[must_use]
  pub const fn new(message: &'static str) -> Self {
    Self { message }
  }

  /// Returns the underlying message.
  #[must_use]
  pub const fn message(&self) -> &'static str {
    self.message
  }
}

impl Display for TaskRunError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    write!(f, "{}", self.message)
  }
}
