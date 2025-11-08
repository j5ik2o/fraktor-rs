//! Serialization infrastructure built on top of extensions.

/// Serializer implementations including the built-in bincode backend.
mod bincode_serializer;
/// Thin wrapper over owned byte buffers.
mod bytes;
/// Error types for serialization failures.
mod error;
/// Extension entry point and ActorSystem integration.
mod extension;
/// Serializable payload container storing manifest metadata.
mod payload;
/// Serializer and manifest registries.
mod registry;
/// Object-safe serializer traits and handles.
mod serializer;
/// Type binding for serialization.
mod type_binding;

pub use bincode_serializer::BincodeSerializer;
pub use bytes::Bytes;
pub use error::SerializationError;
pub use extension::{SERIALIZATION_EXTENSION, Serialization, SerializationExtensionId};
pub use payload::SerializedPayload;
pub use registry::SerializerRegistry;
pub use serializer::{SerializerHandle, SerializerImpl};
