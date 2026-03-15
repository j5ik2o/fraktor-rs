//! Asynchronous serializer trait.

#[cfg(test)]
mod tests;

use alloc::{boxed::Box, vec::Vec};
use core::{any::Any, future::Future, pin::Pin};

use super::error::SerializationError;

/// Future type returned by [`AsyncSerializer`] methods.
pub type SerializationFuture<T> = Pin<Box<dyn Future<Output = Result<T, SerializationError>> + Send>>;

/// Opt-in capability for serializers that perform asynchronous serialization.
///
/// Corresponds to Pekko's `AsyncSerializer`. Useful for serializers that need
/// to interact with external storage or perform CPU-intensive encoding off the
/// calling task.
///
/// Implementations must also implement [`Serializer`](super::serializer::Serializer)
/// to provide synchronous fallbacks. The synchronous methods may block or
/// return an error when the serializer is inherently async-only.
pub trait AsyncSerializer: Send + Sync {
  /// Asynchronously serializes the given message into bytes.
  ///
  /// # Errors
  ///
  /// The returned future resolves to [`SerializationError`] if encoding fails.
  fn to_binary_async(&self, message: Box<dyn Any + Send + Sync>) -> SerializationFuture<Vec<u8>>;

  /// Asynchronously restores a message from bytes with a string manifest.
  ///
  /// # Errors
  ///
  /// The returned future resolves to [`SerializationError`] if decoding fails.
  #[allow(clippy::wrong_self_convention)]
  fn from_binary_async(&self, bytes: Vec<u8>, manifest: &str) -> SerializationFuture<Box<dyn Any + Send + Sync>>;
}
