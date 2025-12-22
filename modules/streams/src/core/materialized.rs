//! Materialized stream result.

use crate::core::mat_combine::MatCombine;

/// Result of materializing a runnable graph.
#[derive(Debug)]
pub struct Materialized<H> {
  handle: H,
  value:  MatCombine,
}

impl<H> Materialized<H> {
  pub(crate) const fn new(handle: H, value: MatCombine) -> Self {
    Self { handle, value }
  }

  /// Returns a reference to the stream handle.
  #[must_use]
  pub const fn handle(&self) -> &H {
    &self.handle
  }

  /// Consumes the materialized result and returns the handle.
  #[must_use]
  pub fn into_handle(self) -> H {
    self.handle
  }

  /// Returns the materialized value.
  #[must_use]
  pub const fn value(&self) -> MatCombine {
    self.value
  }
}
