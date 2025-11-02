use alloc::string::String;

/// Errors returned by the name registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameRegistryError {
  /// The provided name already exists in the registry.
  Duplicate(String),
}
