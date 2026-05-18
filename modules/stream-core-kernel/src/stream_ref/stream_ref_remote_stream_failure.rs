use alloc::string::String;

/// Failure sent by a remote StreamRef endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamRefRemoteStreamFailure {
  message: String,
}

impl StreamRefRemoteStreamFailure {
  /// Creates a remote stream failure protocol payload.
  #[must_use]
  pub const fn new(message: String) -> Self {
    Self { message }
  }

  /// Returns the failure message.
  #[must_use]
  pub fn message(&self) -> &str {
    &self.message
  }
}
