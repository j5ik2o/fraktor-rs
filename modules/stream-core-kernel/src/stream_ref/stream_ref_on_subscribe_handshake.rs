use alloc::string::String;

/// Handshake sent when a StreamRef endpoint subscribes to its partner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamRefOnSubscribeHandshake {
  target_ref_path: String,
}

impl StreamRefOnSubscribeHandshake {
  /// Creates a handshake with the target actor path used by the partner endpoint.
  #[must_use]
  pub const fn new(target_ref_path: String) -> Self {
    Self { target_ref_path }
  }

  /// Returns the serialized target actor path.
  #[must_use]
  pub fn target_ref_path(&self) -> &str {
    &self.target_ref_path
  }
}
