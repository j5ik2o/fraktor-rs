use alloc::string::String;
use core::fmt;

/// Error raised when registering or resolving dispatcher/mailbox identifiers fails.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigError {
  /// Dispatcher identifier already exists.
  DispatcherDuplicate(String),
  /// Dispatcher identifier was not found.
  DispatcherUnknown(String),
  /// Mailbox identifier already exists.
  MailboxDuplicate(String),
  /// Mailbox identifier was not found.
  MailboxUnknown(String),
}

impl ConfigError {
  /// Creates a dispatcher duplicate error.
  #[must_use]
  pub fn dispatcher_duplicate(id: impl Into<String>) -> Self {
    Self::DispatcherDuplicate(id.into())
  }

  /// Creates a dispatcher unknown error.
  #[must_use]
  pub fn dispatcher_unknown(id: impl Into<String>) -> Self {
    Self::DispatcherUnknown(id.into())
  }

  /// Creates a mailbox duplicate error.
  #[must_use]
  pub fn mailbox_duplicate(id: impl Into<String>) -> Self {
    Self::MailboxDuplicate(id.into())
  }

  /// Creates a mailbox unknown error.
  #[must_use]
  pub fn mailbox_unknown(id: impl Into<String>) -> Self {
    Self::MailboxUnknown(id.into())
  }
}

impl fmt::Display for ConfigError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | Self::DispatcherDuplicate(id) => write!(f, "dispatcher id '{}' already exists", id),
      | Self::DispatcherUnknown(id) => write!(f, "dispatcher id '{}' not found", id),
      | Self::MailboxDuplicate(id) => write!(f, "mailbox id '{}' already exists", id),
      | Self::MailboxUnknown(id) => write!(f, "mailbox id '{}' not found", id),
    }
  }
}
