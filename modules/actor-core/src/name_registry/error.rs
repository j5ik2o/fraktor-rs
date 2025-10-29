use alloc::string::String;
use core::fmt;

/// Errors emitted by [`NameRegistry`].
#[derive(Debug, Eq, PartialEq)]
pub enum NameRegistryError {
  /// The provided name already exists within the registry.
  DuplicateName(String),
  /// The provided name was empty or otherwise invalid.
  InvalidName,
}

impl fmt::Display for NameRegistryError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | Self::DuplicateName(name) => write!(f, "actor name '{name}' already exists"),
      | Self::InvalidName => write!(f, "actor name is invalid"),
    }
  }
}
