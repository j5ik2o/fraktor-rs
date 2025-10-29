//! Actor error categorization.

/// Classification for actor execution errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorError {
  /// The failure can be recovered by restarting the actor.
  Recoverable(&'static str),
  /// The failure is considered fatal and should stop the actor tree.
  Fatal(&'static str),
}

impl ActorError {
  /// Creates a recoverable error with the provided reason.
  #[must_use]
  pub const fn recoverable(reason: &'static str) -> Self {
    Self::Recoverable(reason)
  }

  /// Creates a fatal error with the provided reason.
  #[must_use]
  pub const fn fatal(reason: &'static str) -> Self {
    Self::Fatal(reason)
  }

  /// Convenience helper used when a runtime hook is missing.
  #[must_use]
  pub const fn unsupported(operation: &'static str) -> Self {
    Self::Recoverable(operation)
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

  /// Returns the stored reason.
  #[must_use]
  pub const fn reason(&self) -> &'static str {
    match self {
      | Self::Recoverable(reason) | Self::Fatal(reason) => reason,
    }
  }
}
