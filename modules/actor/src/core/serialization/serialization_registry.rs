//! Runtime serialization registry responsible for resolving serializers by type.

mod serialization_registry_generic;
mod serializer_resolution_origin;

#[cfg(test)]
mod tests;

pub use serialization_registry_generic::{SerializationRegistry, SerializationRegistryGeneric};
pub use serializer_resolution_origin::SerializerResolutionOrigin;
