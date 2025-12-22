//! Stage identifier for stream graph components.

use core::sync::atomic::Ordering;

use portable_atomic::AtomicUsize;

/// Unique identifier for a stream stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StageId(u64);

impl StageId {
  /// Creates a new stage identifier from a raw value.
  #[must_use]
  pub const fn new(value: u64) -> Self {
    Self(value)
  }

  /// Returns the raw identifier value.
  #[must_use]
  pub const fn value(self) -> u64 {
    self.0
  }

  /// Generates a monotonically increasing stage identifier.
  #[must_use]
  pub fn next() -> Self {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(1);
    let value = NEXT_ID.fetch_add(1, Ordering::Relaxed) as u64;
    Self(value)
  }
}
