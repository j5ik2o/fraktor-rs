use alloc::string::String;
use core::fmt::{Display, Formatter, Result as FmtResult};

use crate::actor::props::MailboxConfigError;

/// Error raised when registering or resolving mailbox identifiers fails.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MailboxRegistryError {
  /// Mailbox identifier already exists.
  Duplicate(String),
  /// Mailbox identifier was not found.
  Unknown(String),
  /// Mailbox configuration contract violated.
  InvalidConfig(MailboxConfigError),
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

impl From<MailboxConfigError> for MailboxRegistryError {
  fn from(error: MailboxConfigError) -> Self {
    Self::InvalidConfig(error)
  }
}

impl Display for MailboxRegistryError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    match self {
      | Self::Duplicate(id) => write!(f, "mailbox id '{}' already exists", id),
      | Self::Unknown(id) => write!(f, "mailbox id '{}' not found", id),
      | Self::InvalidConfig(error) => write!(f, "invalid mailbox config: {}", error),
    }
  }
}
