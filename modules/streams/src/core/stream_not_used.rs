/// Marker value indicating no materialized value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamNotUsed;

impl StreamNotUsed {
  /// Creates a new marker value.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }
}

impl Default for StreamNotUsed {
  fn default() -> Self {
    Self::new()
  }
}
