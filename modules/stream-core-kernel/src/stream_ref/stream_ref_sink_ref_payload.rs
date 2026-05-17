use alloc::string::String;

/// Serialized payload for a typed [`SinkRef`](super::SinkRef) field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamRefSinkRefPayload {
  actor_path: String,
}

impl StreamRefSinkRefPayload {
  /// Creates a serialized SinkRef payload from a canonical actor path.
  #[must_use]
  pub const fn new(actor_path: String) -> Self {
    Self { actor_path }
  }

  /// Returns the canonical endpoint actor path.
  #[must_use]
  pub fn actor_path(&self) -> &str {
    &self.actor_path
  }
}
