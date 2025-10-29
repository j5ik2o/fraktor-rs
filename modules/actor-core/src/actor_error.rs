//! Error classification used across actor lifecycle callbacks.

use alloc::{borrow::Cow, string::String};
use core::fmt;

/// Classification of failures returned from actor lifecycle hooks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActorError {
  /// Recoverable failure. The supervisor may attempt to restart the actor.
  Recoverable(ActorErrorDetail),
  /// Fatal failure. The actor must be stopped and the error escalated.
  Fatal(ActorErrorDetail),
}

impl ActorError {
  /// Creates a recoverable error with the provided code string.
  #[must_use]
  pub fn recoverable(code: impl Into<Cow<'static, str>>) -> Self {
    Self::Recoverable(ActorErrorDetail::new(code))
  }

  /// Creates a recoverable error with additional detail.
  #[must_use]
  pub fn recoverable_with_detail(code: impl Into<Cow<'static, str>>, detail: impl Into<String>) -> Self {
    Self::Recoverable(ActorErrorDetail::with_detail(code, detail))
  }

  /// Creates a fatal error with the provided code string.
  #[must_use]
  pub fn fatal(code: impl Into<Cow<'static, str>>) -> Self {
    Self::Fatal(ActorErrorDetail::new(code))
  }

  /// Creates a fatal error with additional detail.
  #[must_use]
  pub fn fatal_with_detail(code: impl Into<Cow<'static, str>>, detail: impl Into<String>) -> Self {
    Self::Fatal(ActorErrorDetail::with_detail(code, detail))
  }

  /// Returns `true` when the error is recoverable.
  #[must_use]
  pub const fn is_recoverable(&self) -> bool {
    matches!(self, Self::Recoverable(_))
  }

  /// Returns `true` when the error is fatal.
  #[must_use]
  pub const fn is_fatal(&self) -> bool {
    matches!(self, Self::Fatal(_))
  }

  /// Returns the code associated with the error.
  #[must_use]
  pub fn code(&self) -> &str {
    match self {
      | Self::Recoverable(detail) | Self::Fatal(detail) => detail.code(),
    }
  }

  /// Returns optional human readable detail for the error.
  #[must_use]
  pub fn detail(&self) -> Option<&str> {
    match self {
      | Self::Recoverable(detail) | Self::Fatal(detail) => detail.detail(),
    }
  }
}

impl fmt::Display for ActorError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | Self::Recoverable(detail) => write!(f, "recoverable failure: {detail}"),
      | Self::Fatal(detail) => write!(f, "fatal failure: {detail}"),
    }
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Additional context attached to an [`ActorError`].
pub struct ActorErrorDetail {
  code:   Cow<'static, str>,
  detail: Option<String>,
}

impl ActorErrorDetail {
  #[must_use]
  fn new(code: impl Into<Cow<'static, str>>) -> Self {
    Self { code: code.into(), detail: None }
  }

  #[must_use]
  fn with_detail(code: impl Into<Cow<'static, str>>, detail: impl Into<String>) -> Self {
    Self { code: code.into(), detail: Some(detail.into()) }
  }

  #[must_use]
  fn code(&self) -> &str {
    self.code.as_ref()
  }

  #[must_use]
  fn detail(&self) -> Option<&str> {
    self.detail.as_deref()
  }
}

impl fmt::Display for ActorErrorDetail {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match &self.detail {
      | Some(detail) => write!(f, "{} ({detail})", self.code),
      | None => write!(f, "{}", self.code),
    }
  }
}
