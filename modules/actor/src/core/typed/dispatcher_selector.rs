//! Dispatcher selection strategy for typed props.

#[cfg(test)]
mod tests;

use alloc::string::String;

/// Selects which dispatcher to assign to an actor.
///
/// Inspired by Pekko's `DispatcherSelector` hierarchy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DispatcherSelector {
  /// Uses the system default dispatcher.
  Default,
  /// Uses a dispatcher registered under the given identifier.
  FromConfig(String),
  /// Uses the same dispatcher as the parent actor.
  SameAsParent,
  /// Uses a blocking-friendly dispatcher for IO-heavy actors.
  Blocking,
}

impl DispatcherSelector {
  /// Creates a selector that resolves from a configuration identifier.
  #[must_use]
  pub fn from_config(id: impl Into<String>) -> Self {
    Self::FromConfig(id.into())
  }
}
