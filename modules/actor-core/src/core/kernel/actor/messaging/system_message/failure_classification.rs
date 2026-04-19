//! Classification of actor failures (recoverable vs fatal).

use crate::core::kernel::actor::error::ActorError;

/// Indicates how the actor classified the failure (recoverable/fatal/escalate).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureClassification {
  /// Indicates a recoverable failure that may be addressed via restart.
  Recoverable,
  /// Indicates a fatal error requiring supervisor escalation or stop.
  Fatal,
  /// Indicates an explicit request to delegate the supervision decision to the parent.
  Escalate,
}

impl From<&ActorError> for FailureClassification {
  fn from(value: &ActorError) -> Self {
    match value {
      | ActorError::Recoverable(_) => FailureClassification::Recoverable,
      | ActorError::Fatal(_) => FailureClassification::Fatal,
      | ActorError::Escalate(_) => FailureClassification::Escalate,
    }
  }
}
