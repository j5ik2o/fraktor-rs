//! Core serializer trait.

use alloc::{boxed::Box, vec::Vec};
use core::any::Any;

use super::{error::SerializationError, serializer_id::SerializerId};

/// Represents a synchronous serializer implementation.
pub trait Serializer: Send + Sync {
  /// Returns the stable identifier of the serializer.
  fn identifier(&self) -> SerializerId;

  /// Indicates whether the serializer embeds manifest information.
  fn include_manifest(&self) -> bool;

  /// Converts the provided message into a byte buffer.
  ///
  /// # Errors
  ///
  /// Returns [`SerializationError`] if encoding fails.
  fn to_binary(&self, _message: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, SerializationError>;

  /// Restores a message from its binary representation.
  ///
  /// # Errors
  ///
  /// Returns [`SerializationError`] if decoding fails.
  fn from_binary(&self, _bytes: &[u8], _type_hint: Option<core::any::TypeId>) -> Result<Box<dyn Any + Send>, SerializationError>;

  /// Provides access to the dynamic type used for downcasting.
  fn as_any(&self) -> &(dyn Any + Send + Sync);
}
