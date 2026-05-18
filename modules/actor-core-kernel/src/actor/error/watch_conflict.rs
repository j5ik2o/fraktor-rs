//! Conflict classification for duplicate `watch` / `watch_with` registration.
//!
//! fraktor-rs rejects subsequent registrations whose semantics clash with the
//! existing watch state. This matches Pekko `DeathWatch.scala:126-132`
//! `checkWatchingSame`: Pekko throws `IllegalStateException` when the previous
//! watch message differs from the new one. fraktor-rs surfaces an `Err`
//! carrying one of these variants instead of panicking.

/// Variants describing how a new watch registration collides with the
/// existing entry.
///
/// See [`WatchRegistrationError::Duplicate`](crate::actor::error::WatchRegistrationError::Duplicate).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WatchConflict {
  /// The existing entry is a plain `watch`, and the caller requested
  /// `watch_with(_, message)`. Pekko `DeathWatch.scala:128` flags this as
  /// `None != Some(_)`.
  PlainThenWatchWith,
  /// The existing entry is `watch_with(_, previous)`, and the caller
  /// requested a plain `watch`. Pekko `DeathWatch.scala:128` flags this as
  /// `Some(_) != None`.
  WatchWithThenPlain,
  /// The existing entry is `watch_with(_, previous)`, and the caller
  /// requested another `watch_with(_, new)`. Pekko allows this when
  /// `previous == new`; fraktor-rs rejects it unconditionally because
  /// `AnyMessage` does not implement `PartialEq`
  /// (design Decision 5).
  WatchWithThenWatchWith,
}
