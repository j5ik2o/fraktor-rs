//! Test utilities for serialization setup.

use super::*;

impl SerializationSetup {
  /// Creates an ad-hoc setup for tests without passing through the builder.
  #[must_use]
  #[allow(clippy::too_many_arguments)]
  pub fn testing_from_raw(
    serializers: HashMap<SerializerId, ArcShared<dyn Serializer>>,
    bindings: HashMap<TypeId, SerializerId>,
    binding_names: HashMap<TypeId, String>,
    remote_manifests: HashMap<TypeId, String>,
    manifest_routes: HashMap<String, Vec<(u8, SerializerId)>>,
    scopes: Vec<SerializationCallScope>,
    fallback: SerializerId,
    adapter_metadata: Vec<String>,
  ) -> Self {
    Self { serializers, bindings, binding_names, remote_manifests, manifest_routes, scopes, fallback, adapter_metadata }
  }
}
