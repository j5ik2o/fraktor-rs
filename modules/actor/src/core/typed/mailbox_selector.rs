//! Mailbox selection strategy for typed props.

#[cfg(test)]
mod tests;

use alloc::string::String;
use core::num::NonZeroUsize;

/// Selects which mailbox type to assign to an actor.
///
/// Inspired by Pekko's `MailboxSelector` hierarchy.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum MailboxSelector {
  /// Uses the system default unbounded mailbox.
  #[default]
  Default,
  /// Uses a bounded mailbox with the specified capacity.
  Bounded(NonZeroUsize),
  /// Uses a mailbox registered under the given identifier.
  FromConfig(String),
}

impl MailboxSelector {
  /// Creates a bounded mailbox selector with the given capacity.
  #[must_use]
  pub const fn bounded(capacity: NonZeroUsize) -> Self {
    Self::Bounded(capacity)
  }

  /// Creates a selector that resolves from a configuration identifier.
  #[must_use]
  pub fn from_config(id: impl Into<String>) -> Self {
    Self::FromConfig(id.into())
  }
}
