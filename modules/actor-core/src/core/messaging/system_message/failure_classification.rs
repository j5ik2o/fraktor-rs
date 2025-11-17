//! Classification of actor failures (recoverable vs fatal).

use crate::core::error::ActorError;

/// Indicates how the actor classified the failure (recoverable/fatal).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureClassification {
  /// Indicates a recoverable failure that may be addressed via restart.
  Recoverable,
  /// Indicates a fatal error requiring supervisor escalation or stop.
  Fatal,
}

impl From<&ActorError> for FailureClassification {
  fn from(value: &ActorError) -> Self {
    match value {
      | ActorError::Recoverable(_) => FailureClassification::Recoverable,
      | ActorError::Fatal(_) => FailureClassification::Fatal,
    }
  }
}
