//! Error returned when registering additional top-level actors fails.

use alloc::string::String;

/// Describes why extra top-level registration could not be completed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegisterExtraTopLevelError {
  /// ActorSystem startup already finished; extra registration is no longer accepted.
  AlreadyStarted,
  /// The provided name conflicts with a reserved system path (e.g., `user`).
  ReservedName(String),
  /// The provided name was already registered.
  DuplicateName(String),
}
