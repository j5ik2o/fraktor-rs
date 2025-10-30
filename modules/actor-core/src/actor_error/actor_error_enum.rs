use super::actor_error_reason::ActorErrorReason;

/// Error classification returned by actor lifecycle callbacks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActorError {
  /// Recoverable failure that allows supervisor strategies to attempt a restart.
  Recoverable(ActorErrorReason),
  /// Fatal failure that stops the actor and propagates to supervisors.
  Fatal(ActorErrorReason),
}

impl ActorError {
  /// Creates a recoverable error with the provided reason.
  #[must_use]
  pub fn recoverable(reason: impl Into<ActorErrorReason>) -> Self {
    Self::Recoverable(reason.into())
  }

  /// Creates a fatal error with the provided reason.
  #[must_use]
  pub fn fatal(reason: impl Into<ActorErrorReason>) -> Self {
    Self::Fatal(reason.into())
  }

  /// Returns the underlying reason regardless of classification.
  #[must_use]
  pub const fn reason(&self) -> &ActorErrorReason {
    match self {
      | ActorError::Recoverable(reason) | ActorError::Fatal(reason) => reason,
    }
  }

  /// Maps the error into a fatal classification while keeping the same reason.
  #[must_use]
  pub fn into_fatal(self) -> Self {
    match self {
      | ActorError::Recoverable(reason) | ActorError::Fatal(reason) => ActorError::Fatal(reason),
    }
  }

  /// Maps the error into a recoverable classification while keeping the same reason.
  #[must_use]
  pub fn into_recoverable(self) -> Self {
    match self {
      | ActorError::Recoverable(reason) | ActorError::Fatal(reason) => ActorError::Recoverable(reason),
    }
  }
}
