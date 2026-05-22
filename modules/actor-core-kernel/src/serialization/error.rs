//! Serialization runtime errors.

#[cfg(test)]
#[path = "error_test.rs"]
mod tests;

use alloc::string::String;

use super::{
  call_scope::SerializationCallScope, not_serializable_error::NotSerializableError, serializer_id::SerializerId,
};

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
  /// Serializer id is already occupied by another serializer.
  SerializerIdCollision(SerializerId),
  /// Type binding is already assigned to another serializer id.
  SerializerBindingCollision {
    /// Bound type name.
    type_name: String,
    /// Existing serializer id.
    existing:  SerializerId,
    /// Requested serializer id.
    requested: SerializerId,
  },
  /// Requested type could not be serialized with the available registry configuration.
  NotSerializable(NotSerializableError),
  /// Manifest string was not recognised.
  UnknownManifest(String),
  /// Serialized payload could not be decoded.
  InvalidFormat,
}

impl SerializationError {
  /// Creates an uninitialized error.
  #[must_use]
  pub const fn uninitialized() -> Self {
    Self::Uninitialized
  }

  /// Creates a manifest missing error for the specified scope.
  #[must_use]
  pub const fn manifest_missing(scope: SerializationCallScope) -> Self {
    Self::ManifestMissing { scope }
  }

  /// Creates an unknown serializer error.
  #[must_use]
  pub const fn unknown_serializer(id: SerializerId) -> Self {
    Self::UnknownSerializer(id)
  }

  /// Creates a serializer id collision error.
  #[must_use]
  pub const fn serializer_id_collision(id: SerializerId) -> Self {
    Self::SerializerIdCollision(id)
  }

  /// Creates a serializer binding collision error.
  #[must_use]
  pub fn serializer_binding_collision(
    type_name: impl Into<String>,
    existing: SerializerId,
    requested: SerializerId,
  ) -> Self {
    Self::SerializerBindingCollision { type_name: type_name.into(), existing, requested }
  }

  /// Creates a not serializable error with the provided details.
  #[must_use]
  pub const fn not_serializable(error: NotSerializableError) -> Self {
    Self::NotSerializable(error)
  }

  /// Creates an unknown manifest error.
  #[must_use]
  pub fn unknown_manifest(manifest: impl Into<String>) -> Self {
    Self::UnknownManifest(manifest.into())
  }

  /// Creates an invalid format error.
  #[must_use]
  pub const fn invalid_format() -> Self {
    Self::InvalidFormat
  }

  /// Returns `true` if the error is `Uninitialized`.
  #[must_use]
  pub const fn is_uninitialized(&self) -> bool {
    matches!(self, Self::Uninitialized)
  }

  /// Returns `true` if the error is `ManifestMissing`.
  #[must_use]
  pub const fn is_manifest_missing(&self) -> bool {
    matches!(self, Self::ManifestMissing { .. })
  }

  /// Returns `true` if the error is `UnknownSerializer`.
  #[must_use]
  pub const fn is_unknown_serializer(&self) -> bool {
    matches!(self, Self::UnknownSerializer(_))
  }

  /// Returns `true` if the error is `SerializerIdCollision`.
  #[must_use]
  pub const fn is_serializer_id_collision(&self) -> bool {
    matches!(self, Self::SerializerIdCollision(_))
  }

  /// Returns `true` if the error is `SerializerBindingCollision`.
  #[must_use]
  pub const fn is_serializer_binding_collision(&self) -> bool {
    matches!(self, Self::SerializerBindingCollision { .. })
  }

  /// Returns `true` if the error is `NotSerializable`.
  #[must_use]
  pub const fn is_not_serializable(&self) -> bool {
    matches!(self, Self::NotSerializable(_))
  }

  /// Returns `true` if the error is `UnknownManifest`.
  #[must_use]
  pub const fn is_unknown_manifest(&self) -> bool {
    matches!(self, Self::UnknownManifest(_))
  }

  /// Returns `true` if the error is `InvalidFormat`.
  #[must_use]
  pub const fn is_invalid_format(&self) -> bool {
    matches!(self, Self::InvalidFormat)
  }
}
