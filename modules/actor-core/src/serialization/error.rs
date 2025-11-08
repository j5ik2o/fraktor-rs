use alloc::string::String;
use core::fmt;

/// Errors originating from serialization subsystems.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SerializationError {
  /// Serializer identifier is already registered.
  DuplicateSerializerId(u32),
  /// Manifest is not known by the target serializer.
  UnknownManifest {
    /// Identifier of the serializer that failed to decode the payload.
    serializer_id: u32,
    /// Manifest accompanying the payload.
    manifest:      String,
  },
  /// Manifest string is invalid or already registered.
  InvalidManifest(String),
  /// Serializer identifier could not be resolved.
  SerializerNotFound(u32),
  /// Type-level serializer is missing.
  NoSerializerForType(&'static str),
  /// Serialization failed.
  SerializationFailed(String),
  /// Deserialization failed.
  DeserializationFailed(String),
  /// Manifest/type mismatch detected.
  TypeMismatch {
    /// Manifest that was expected based on registry contents.
    expected: String,
    /// Manifest observed on the incoming payload.
    found:    String,
  },
}

impl fmt::Display for SerializationError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | Self::DuplicateSerializerId(id) => write!(f, "serializer id {id} already registered"),
      | Self::UnknownManifest { serializer_id, manifest } => {
        write!(f, "unknown manifest '{manifest}' for serializer {serializer_id}")
      },
      | Self::InvalidManifest(manifest) => write!(f, "invalid manifest '{manifest}'"),
      | Self::SerializerNotFound(id) => write!(f, "serializer {id} not found"),
      | Self::NoSerializerForType(ty) => write!(f, "no serializer registered for type {ty}"),
      | Self::SerializationFailed(reason) => write!(f, "serialization failed: {reason}"),
      | Self::DeserializationFailed(reason) => write!(f, "deserialization failed: {reason}"),
      | Self::TypeMismatch { expected, found } => {
        write!(f, "type mismatch; expected '{expected}' but found '{found}'")
      },
    }
  }
}
