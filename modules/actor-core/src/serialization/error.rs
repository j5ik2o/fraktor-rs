//! Serialization runtime errors.

#[cfg(test)]
mod tests;

use alloc::string::String;

use super::{call_scope::SerializationCallScope, not_serializable_error::NotSerializableError, serializer_id::SerializerId};

/// Errors emitted by serialization operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SerializationError {
  /// An operation was attempted after shutdown.
  Uninitialized,
  /// Manifest was required but missing for the specified scope.
  ManifestMissing {
    /// Scope that requires an explicit manifest.
    scope: SerializationCallScope,
  },
  /// Serializer lookup failed for the provided identifier.
  UnknownSerializer(SerializerId),
  /// Requested type could not be serialized with the available registry configuration.
  NotSerializable(NotSerializableError),
  /// Manifest string was not recognised.
  UnknownManifest(String),
  /// Serialized payload could not be decoded.
  InvalidFormat,
}
