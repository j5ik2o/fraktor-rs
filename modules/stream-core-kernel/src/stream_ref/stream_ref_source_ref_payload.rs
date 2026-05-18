use alloc::string::String;

/// Serialized payload for a typed [`SourceRef`](super::SourceRef) field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamRefSourceRefPayload {
  actor_path: String,
}

impl StreamRefSourceRefPayload {
  /// Creates a serialized SourceRef payload from a canonical actor path.
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
