use core::fmt;

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

impl fmt::Display for TaskRunError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.message)
  }
}
