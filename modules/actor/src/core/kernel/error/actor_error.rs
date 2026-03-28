//! Error classification returned by actor lifecycle callbacks.

#[cfg(test)]
mod tests;

use alloc::{borrow::Cow, format};

use crate::core::kernel::error::{SendError, actor_error_reason::ActorErrorReason};

/// Categorizes actor failures and informs supervision decisions.
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

  /// Converts this error into a fatal classification.
  #[must_use]
  pub fn into_fatal(self) -> Self {
    match self {
      | ActorError::Recoverable(reason) | ActorError::Fatal(reason) => ActorError::Fatal(reason),
    }
  }

  /// Converts this error into a recoverable classification.
  #[must_use]
  pub fn into_recoverable(self) -> Self {
    match self {
      | ActorError::Recoverable(reason) | ActorError::Fatal(reason) => ActorError::Recoverable(reason),
    }
  }

  /// Creates a recoverable error tagged with the source error type.
  #[must_use]
  pub fn recoverable_typed<E: 'static>(reason: impl Into<Cow<'static, str>>) -> Self {
    Self::Recoverable(ActorErrorReason::typed::<E>(reason))
  }

  /// Creates a fatal error tagged with the source error type.
  #[must_use]
  pub fn fatal_typed<E: 'static>(reason: impl Into<Cow<'static, str>>) -> Self {
    Self::Fatal(ActorErrorReason::typed::<E>(reason))
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
