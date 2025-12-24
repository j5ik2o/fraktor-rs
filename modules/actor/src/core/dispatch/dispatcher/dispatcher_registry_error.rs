use alloc::string::String;
use core::fmt;

/// Error raised when registering or resolving dispatcher identifiers fails.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DispatcherRegistryError {
  /// Dispatcher identifier already exists.
  Duplicate(String),
  /// Dispatcher identifier was not found.
  Unknown(String),
}

impl DispatcherRegistryError {
  /// Creates a dispatcher duplicate error.
  #[must_use]
  pub fn duplicate(id: impl Into<String>) -> Self {
    Self::Duplicate(id.into())
  }

  /// Creates a dispatcher unknown error.
  #[must_use]
  pub fn unknown(id: impl Into<String>) -> Self {
    Self::Unknown(id.into())
  }
}

impl fmt::Display for DispatcherRegistryError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | Self::Duplicate(id) => write!(f, "dispatcher id '{}' already exists", id),
      | Self::Unknown(id) => write!(f, "dispatcher id '{}' not found", id),
    }
  }
}
