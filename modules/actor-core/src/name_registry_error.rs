//! Errors produced by the name registry.

use alloc::string::String;

/// Error variants emitted when managing name registrations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NameRegistryError {
  /// The provided name already exists in the registry.
  Duplicate(String),
}

#[cfg(test)]
mod tests;
