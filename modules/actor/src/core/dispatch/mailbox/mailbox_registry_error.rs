use alloc::string::String;
use core::fmt;

/// Error raised when registering or resolving mailbox identifiers fails.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MailboxRegistryError {
  /// Mailbox identifier already exists.
  Duplicate(String),
  /// Mailbox identifier was not found.
  Unknown(String),
}

impl MailboxRegistryError {
  /// Creates a mailbox duplicate error.
  #[must_use]
  pub fn duplicate(id: impl Into<String>) -> Self {
    Self::Duplicate(id.into())
  }

  /// Creates a mailbox unknown error.
  #[must_use]
  pub fn unknown(id: impl Into<String>) -> Self {
    Self::Unknown(id.into())
  }
}

impl fmt::Display for MailboxRegistryError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | Self::Duplicate(id) => write!(f, "mailbox id '{}' already exists", id),
      | Self::Unknown(id) => write!(f, "mailbox id '{}' not found", id),
    }
  }
}
