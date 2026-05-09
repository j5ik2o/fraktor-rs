//! Tri-state describing whether a pid has a user-level watch registration,
//! and if so whether a custom termination message is attached.
//!
//! Used by [`ActorCell::watch_registration_kind`](crate::actor::ActorCell::watch_registration_kind)
//! to drive the Pekko-parity duplicate check in
//! `ActorContext::watch` / `watch_with`.
//!
//! The three states mirror Pekko `DeathWatch.scala:30`'s
//! `watching: Map[ActorRef, Option[Any]]`:
//!
//! | fraktor-rs                    | Pekko                   |
//! |-------------------------------|-------------------------|
//! | `WatchRegistrationKind::None` | absent from `watching`  |
//! | `WatchRegistrationKind::Plain`| `watching(ref) = None`  |
//! | `WatchRegistrationKind::WithMessage` | `watching(ref) = Some(_)` |
//!
//! Supervision-only watches (`WatchKind::Supervision`) are deliberately
//! ignored by the query because they are kernel-internal parent/child
//! bookkeeping and are not subject to user-level duplicate detection.

/// Current user-level watch registration state for a given pid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WatchRegistrationKind {
  /// No user watch is registered. `watching` may still contain a
  /// `WatchKind::Supervision` entry for the same pid; that is irrelevant
  /// here.
  None,
  /// A plain `watch` (no custom message) is registered.
  Plain,
  /// A `watch_with(_, msg)` is registered and `msg` is stored in
  /// `watch_with_messages`.
  WithMessage,
}
