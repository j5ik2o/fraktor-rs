//! Unique identifier for adapter reference handles.

/// Unique identifier associated with a registered adapter reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdapterRefHandleId(u64);

impl AdapterRefHandleId {
  /// Creates a new identifier from the provided numeric value.
  #[must_use]
  pub const fn new(value: u64) -> Self {
    Self(value)
  }

  /// Returns the raw numeric representation.
  #[must_use]
  pub const fn get(&self) -> u64 {
    self.0
  }
}
