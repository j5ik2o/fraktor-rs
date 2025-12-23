/// Marker value indicating stream completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamDone;

impl StreamDone {
  /// Creates a new marker value.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }
}

impl Default for StreamDone {
  fn default() -> Self {
    Self::new()
  }
}
