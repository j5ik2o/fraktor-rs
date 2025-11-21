//! Version clock utilities for membership updates.

/// Monotonic version used to order membership updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MembershipVersion(pub u64);

impl MembershipVersion {
  /// Returns the zero version.
  pub const fn zero() -> Self {
    Self(0)
  }

  /// Creates a new version with an explicit value.
  pub const fn new(value: u64) -> Self {
    Self(value)
  }

  /// Extracts the inner value.
  pub const fn value(self) -> u64 {
    self.0
  }

  /// Returns the next version.
  pub const fn next(self) -> Self {
    Self(self.0 + 1)
  }
}
