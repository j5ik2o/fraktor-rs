/// Key for identifying items registered in a DeadlineTimer.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, Default)]
pub struct DeadlineTimerKey(u64);

impl DeadlineTimerKey {
  /// Returns an invalid key.
  #[must_use]
  #[inline]
  pub const fn invalid() -> Self {
    Self(0)
  }

  /// Checks if the key is valid.
  #[must_use]
  #[inline]
  pub const fn is_valid(self) -> bool {
    self.0 != 0
  }

  /// Retrieves the internal representation.
  #[must_use]
  #[inline]
  pub const fn into_raw(self) -> u64 {
    self.0
  }

  /// Creates a key from the raw representation.
  #[must_use]
  #[inline]
  pub const fn from_raw(raw: u64) -> Self {
    Self(raw)
  }
}
