//! Buffer-reuse serializer trait.

#[cfg(test)]
mod tests;

use alloc::{boxed::Box, vec::Vec};
use core::any::Any;

use super::error::SerializationError;

/// Opt-in capability for serializers that can write into a caller-owned buffer.
///
/// Corresponds to Pekko's `ByteBufferSerializer`. Implementations that also
/// implement [`Serializer`](super::serializer::Serializer) can override the
/// default byte-array methods by delegating to these buffer-based methods.
///
/// The caller provides a mutable `Vec<u8>` that the serializer appends to,
/// avoiding an extra allocation when the buffer is reused across calls.
pub trait ByteBufferSerializer: Send + Sync {
  /// Serializes the given message by appending bytes to the provided buffer.
  ///
  /// The serializer must **not** clear the buffer; it should only append.
  ///
  /// # Errors
  ///
  /// Returns [`SerializationError`] if encoding fails.
  fn to_binary_buf(&self, message: &(dyn Any + Send + Sync), buf: &mut Vec<u8>) -> Result<(), SerializationError>;

  /// Restores a message from the provided byte slice with a string manifest.
  ///
  /// # Errors
  ///
  /// Returns [`SerializationError`] if decoding fails.
  #[allow(clippy::wrong_self_convention)]
  fn from_binary_buf(&self, bytes: &[u8], manifest: &str) -> Result<Box<dyn Any + Send + Sync>, SerializationError>;
}
