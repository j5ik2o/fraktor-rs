//! Errors returned by `ActorContext::watch` / `watch_with`.

use alloc::fmt::{Debug, Formatter, Result as FmtResult};

use super::{ActorError, SendError, WatchConflict};
use crate::core::kernel::actor::Pid;

/// Error returned by `ActorContext::watch` / `watch_with`.
///
/// Wraps the underlying send failure while surfacing duplicate-registration
/// conflicts that Pekko `DeathWatch.scala:126-132` (`checkWatchingSame`)
/// signals via `IllegalStateException`.
pub enum WatchRegistrationError {
  /// Failed to enqueue the underlying `Watch` system message.
  Send(SendError),
  /// The caller tried to register a watch with semantics incompatible with
  /// the existing entry. See [`WatchConflict`] for the classification.
  Duplicate {
    /// Pid of the target that already has a conflicting watch entry.
    target:   Pid,
    /// How the new registration clashes with the previous one.
    conflict: WatchConflict,
  },
}

impl WatchRegistrationError {
  /// Creates an error wrapping the underlying send failure.
  #[must_use]
  pub const fn send(error: SendError) -> Self {
    Self::Send(error)
  }

  /// Creates an error describing a duplicate watch registration.
  #[must_use]
  pub const fn duplicate(target: Pid, conflict: WatchConflict) -> Self {
    Self::Duplicate { target, conflict }
  }

  /// Converts this error into a recoverable [`ActorError`] suitable for
  /// propagating through supervised caller sites.
  ///
  /// - [`Self::Send`] delegates to [`ActorError::from_send_error`] so the existing reason-carrying
  ///   behaviour is preserved.
  /// - [`Self::Duplicate`] becomes a recoverable `ActorError` describing the target pid and
  ///   conflict kind. Guardian actors remain alive and their supervisor decides the response.
  #[must_use]
  pub fn to_actor_error(&self) -> ActorError {
    match self {
      | Self::Send(error) => ActorError::from_send_error(error),
      | Self::Duplicate { target, conflict } => {
        ActorError::recoverable(alloc::format!("duplicate watch registration on {target:?}: {conflict:?}"))
      },
    }
  }
}

impl From<SendError> for WatchRegistrationError {
  fn from(error: SendError) -> Self {
    Self::Send(error)
  }
}

impl Debug for WatchRegistrationError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    match self {
      | Self::Send(inner) => f.debug_tuple("Send").field(inner).finish(),
      | Self::Duplicate { target, conflict } => {
        f.debug_struct("Duplicate").field("target", target).field("conflict", conflict).finish()
      },
    }
  }
}
