use alloc::string::String;

/// Error returned by a deployable factory registered on the target node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeployableFactoryError {
  reason: String,
}

impl DeployableFactoryError {
  /// Creates a factory error with a human-readable reason.
  #[must_use]
  pub fn new(reason: impl Into<String>) -> Self {
    Self { reason: reason.into() }
  }

  /// Returns the human-readable rejection reason.
  #[must_use]
  pub fn reason(&self) -> &str {
    &self.reason
  }
}
