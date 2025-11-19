//! Serialization subsystem aggregation.

mod builder;
mod builder_error;
mod builtin;
mod call_scope;
mod config_adapter;
mod default_setup;
mod delegator;
mod error;
mod error_event;
mod extension;
mod not_serializable_error;
mod serialization_extension_id;
mod serialization_extension_installer;
mod serialization_registry;
mod serialization_setup;
mod serialized_message;
mod serializer;
mod serializer_id;
mod serializer_id_error;
mod string_manifest_serializer;
mod transport_information;

// Re-exports from builder
pub use builder::SerializationSetupBuilder;
// Re-exports from builder_error
pub use builder_error::SerializationBuilderError;
// Re-exports from builtin
pub use builtin::{
  BOOL_ID, BYTES_ID, BoolSerializer, BytesSerializer, I32_ID, I32Serializer, NULL_ID, NullSerializer, STRING_ID,
  StringSerializer, register_defaults,
};
// Re-exports from call_scope
pub use call_scope::SerializationCallScope;
// Re-exports from config_adapter
pub use config_adapter::SerializationConfigAdapter;
// Re-exports from default_setup
pub use default_setup::{default_serialization_extension_id, default_serialization_setup};
// Re-exports from delegator
pub use delegator::SerializationDelegator;
// Re-exports from error
pub use error::SerializationError;
// Re-exports from error_event
pub use error_event::SerializationErrorEvent;
// Re-exports from extension
pub use extension::{SerializationExtension, SerializationExtensionGeneric};
// Re-exports from not_serializable_error
pub use not_serializable_error::NotSerializableError;
// Re-exports from serialization_extension_id
pub use serialization_extension_id::SerializationExtensionId;
// Re-exports from serialization_extension_installer
pub use serialization_extension_installer::SerializationExtensionInstaller;
// Re-exports from serialization_registry
pub use serialization_registry::{SerializationRegistry, SerializationRegistryGeneric, SerializerResolutionOrigin};
// Re-exports from serialization_setup
pub use serialization_setup::SerializationSetup;
// Re-exports from serialized_message
pub use serialized_message::SerializedMessage;
// Re-exports from serializer
pub use serializer::Serializer;
// Re-exports from serializer_id
pub use serializer_id::SerializerId;
// Re-exports from serializer_id_error
pub use serializer_id_error::SerializerIdError;
// Re-exports from string_manifest_serializer
pub use string_manifest_serializer::SerializerWithStringManifest;
// Re-exports from transport_information
pub use transport_information::TransportInformation;
