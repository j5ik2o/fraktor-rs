//! Runtime serialization registry responsible for resolving serializers by type.

mod registry;
mod serializer_resolution_origin;

#[cfg(test)]
mod tests;

pub use registry::SerializationRegistry;
pub use serializer_resolution_origin::SerializerResolutionOrigin;
