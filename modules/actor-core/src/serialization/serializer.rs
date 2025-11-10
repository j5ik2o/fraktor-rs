//! Core serializer trait.

use alloc::{boxed::Box, vec::Vec};
use core::any::{Any, TypeId};

use super::{
  error::SerializationError, serializer_id::SerializerId, string_manifest_serializer::SerializerWithStringManifest,
};

/// Represents a synchronous serializer implementation.
pub trait Serializer: Send + Sync {
  /// Returns the stable identifier of the serializer.
  fn identifier(&self) -> SerializerId;

  /// Indicates whether the serializer embeds manifest information.
  ///
  /// Defaults to `false`.
  fn include_manifest(&self) -> bool {
    false
  }

  /// Converts the provided message into a byte buffer.
  ///
  /// # Errors
  ///
  /// Returns [`SerializationError`] if encoding fails.
  fn to_binary(&self, message: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, SerializationError>;

  /// Restores a message from its binary representation.
  ///
  /// # Errors
  ///
  /// Returns [`SerializationError`] if decoding fails.
  fn from_binary(&self, bytes: &[u8], type_hint: Option<TypeId>) -> Result<Box<dyn Any + Send>, SerializationError>;

  /// Provides access to the dynamic type used for downcasting.
  fn as_any(&self) -> &(dyn Any + Send + Sync);

  /// Returns a reference to the [`SerializerWithStringManifest`] view if implemented.
  ///
  /// Defaults to `None`.
  fn as_string_manifest(&self) -> Option<&dyn SerializerWithStringManifest> {
    None
  }
}
