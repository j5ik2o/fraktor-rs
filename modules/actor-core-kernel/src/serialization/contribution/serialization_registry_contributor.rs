//! Serialization registry contributor trait.

use fraktor_utils_core_rs::sync::ArcShared;

use crate::serialization::{SerializationError, serialization_registry::SerializationRegistry};

/// Adds serializers and bindings to a live serialization registry.
pub trait SerializationRegistryContributor: Send + Sync {
  /// Applies this contribution to the registry.
  ///
  /// # Errors
  ///
  /// Returns [`SerializationError`] when serializer or binding registration fails.
  fn contribute(&self, registry: &ArcShared<SerializationRegistry>) -> Result<(), SerializationError>;
}
