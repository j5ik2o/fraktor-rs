//! Builder error types for serialization setup.

use alloc::string::String;

use super::{call_scope::SerializationCallScope, serializer_id::SerializerId};

/// Errors that can occur while constructing the serialization setup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SerializationBuilderError {
  /// Serializer identifier collides with the runtime reserved range.
  ReservedIdentifier(u32),
  /// Duplicate identifier detected.
  DuplicateIdentifier(SerializerId),
  /// Duplicate name detected.
  DuplicateName(String),
  /// Attempted to bind a marker type multiple times.
  DuplicateMarker(String),
  /// Attempted to use a marker before registering a binding.
  MarkerUnbound(String),
  /// A required manifest rule was not satisfied for the specified scope.
  ManifestRequired(SerializationCallScope),
  /// No fallback serializer was configured.
  MissingFallback,
  /// Serializer referenced by name was not registered.
  UnknownSerializer(String),
  /// Duplicate manifest declaration detected for the type.
  DuplicateManifestBinding(String),
  /// Duplicate manifest route priority detected for the manifest string.
  ManifestRouteDuplicate {
    /// Manifest string that already contains the specified priority.
    manifest: String,
    /// Priority that caused the collision.
    priority: u8,
  },
}
