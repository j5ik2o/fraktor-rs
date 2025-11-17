//! Error classification returned by actor lifecycle callbacks.

#[cfg(test)]
mod tests;

use alloc::format;

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::error::{SendError, actor_error_reason::ActorErrorReason};

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

  /// Creates a recoverable actor error from a send failure.
  #[must_use]
  pub fn from_send_error<TB: RuntimeToolbox>(error: &SendError<TB>) -> Self {
    ActorError::recoverable(format!("send failed: {:?}", error))
  }
}
