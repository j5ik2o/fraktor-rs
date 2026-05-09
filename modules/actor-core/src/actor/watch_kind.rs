//! Distinguishes user-requested death watches from kernel-internal parent/child
//! supervision watches.
//!
//! fraktor-rs kernels register two kinds of watches against the same
//! [`ActorCellState`](crate::actor::ActorCellState):
//!
//! * [`WatchKind::User`] — created through `ActorContext::watch` / `watch_with`. Removable via
//!   `ActorContext::unwatch`.
//! * [`WatchKind::Supervision`] — created automatically by `spawn_with_parent` so that the parent
//!   receives the child's
//!   [`DeathWatchNotification`](crate::actor::messaging::SystemMessage::DeathWatchNotification).
//!   This kind is **not** removed by user-level `unwatch`.
//!
//! Tagging each entry with its kind prevents user `unwatch` calls from
//! accidentally detaching the internal supervision watch that drives
//! `finish_recreate` / `finish_terminate`.

/// Origin of a watch relationship.
///
/// See the module-level doc for the invariants enforced by the distinction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WatchKind {
  /// Registered via `ActorContext::watch` / `watch_with`.
  User,
  /// Registered automatically by `spawn_with_parent` for parent/child
  /// supervision. Immune to user-level `unwatch`.
  Supervision,
}
