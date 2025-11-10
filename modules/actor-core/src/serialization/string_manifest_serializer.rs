//! Serializer extension trait for string manifests.

use alloc::{borrow::Cow, boxed::Box};

use super::serializer::Serializer;

/// Serializer flavour that attaches a string manifest to each payload.
pub trait SerializerWithStringManifest: Serializer {
  /// Returns the manifest string for the provided message.
  fn manifest(&self, message: &(dyn core::any::Any + Send + Sync)) -> Cow<'_, str>;

  /// Restores a message from bytes using the provided manifest.
  fn from_binary_with_manifest(&self, bytes: &[u8], manifest: &str) -> Result<Box<dyn core::any::Any + Send>, super::error::SerializationError>;
}
