//! Error classification returned by actor lifecycle callbacks.

#[cfg(test)]
mod tests;

use alloc::{borrow::Cow, format};

use crate::actor::error::{SendError, actor_error_reason::ActorErrorReason};

/// Categorizes actor failures and informs supervision decisions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActorError {
  /// Recoverable failure that allows supervisor strategies to attempt a restart.
  Recoverable(ActorErrorReason),
  /// Fatal failure that stops the actor and propagates to supervisors.
  Fatal(ActorErrorReason),
  /// Escalation request delegating the supervision decision to the parent supervisor
  /// (Pekko parity: mirrors `Error` escalation in `defaultDecider`).
  Escalate(ActorErrorReason),
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

  /// Creates an escalation request that delegates the supervision decision to the parent
  /// supervisor.
  #[must_use]
  pub fn escalate(reason: impl Into<ActorErrorReason>) -> Self {
    Self::Escalate(reason.into())
  }

  /// Returns the underlying reason regardless of classification.
  #[must_use]
  pub const fn reason(&self) -> &ActorErrorReason {
    match self {
      | ActorError::Recoverable(reason) | ActorError::Fatal(reason) | ActorError::Escalate(reason) => reason,
    }
  }

  /// Returns a cloned copy of the underlying reason.
  #[must_use]
  pub fn to_reason(&self) -> ActorErrorReason {
    self.reason().clone()
  }

  /// Converts this error into a fatal classification.
  #[must_use]
  pub fn into_fatal(self) -> Self {
    match self {
      | ActorError::Recoverable(reason) | ActorError::Fatal(reason) | ActorError::Escalate(reason) => {
        ActorError::Fatal(reason)
      },
    }
  }

  /// Converts this error into a recoverable classification.
  #[must_use]
  pub fn into_recoverable(self) -> Self {
    match self {
      | ActorError::Recoverable(reason) | ActorError::Fatal(reason) | ActorError::Escalate(reason) => {
        ActorError::Recoverable(reason)
      },
    }
  }

  /// Creates a recoverable error tagged with the source error type.
  #[must_use]
  pub fn recoverable_typed<E: 'static>(reason: impl Into<Cow<'static, str>>) -> Self {
    Self::Recoverable(ActorErrorReason::with_source_type::<E>(reason))
  }

  /// Creates a fatal error tagged with the source error type.
  #[must_use]
  pub fn fatal_typed<E: 'static>(reason: impl Into<Cow<'static, str>>) -> Self {
    Self::Fatal(ActorErrorReason::with_source_type::<E>(reason))
  }

  /// Returns `true` when the source error matches the provided type.
  #[must_use]
  pub fn is_source_type<E: 'static>(&self) -> bool {
    self.reason().is_source_type::<E>()
  }

  /// Creates a recoverable actor error from a send failure.
  #[must_use]
  pub fn from_send_error(error: &SendError) -> Self {
    ActorError::recoverable(format!("send failed: {:?}", error))
  }
}
