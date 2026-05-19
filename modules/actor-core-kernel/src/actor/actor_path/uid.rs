//! UID wrapper used by canonical actor paths.

/// Unique identifier appended to canonical actor paths.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ActorUid(u64);

impl ActorUid {
  /// Creates a new UID wrapper.
  #[must_use]
  pub const fn new(value: u64) -> Self {
    Self(value)
  }

  /// Returns the inner UID value.
  #[must_use]
  pub const fn value(&self) -> u64 {
    self.0
  }
}
