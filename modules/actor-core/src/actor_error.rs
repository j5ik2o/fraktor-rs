use alloc::{borrow::Cow, string::String};
use core::fmt;

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
  pub fn reason(&self) -> &ActorErrorReason {
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

/// Human readable explanation associated with an actor error.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ActorErrorReason(Cow<'static, str>);

impl ActorErrorReason {
  /// Creates a new reason.
  #[must_use]
  pub fn new(reason: impl Into<Cow<'static, str>>) -> Self {
    Self(reason.into())
  }

  /// Returns the message.
  #[must_use]
  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl From<&'static str> for ActorErrorReason {
  fn from(value: &'static str) -> Self {
    Self(Cow::Borrowed(value))
  }
}

impl From<String> for ActorErrorReason {
  fn from(value: String) -> Self {
    Self(Cow::Owned(value))
  }
}

impl From<Cow<'static, str>> for ActorErrorReason {
  fn from(value: Cow<'static, str>) -> Self {
    Self(value)
  }
}

impl fmt::Display for ActorErrorReason {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}
