//! Serialization subsystem aggregation.

mod builder;
mod builder_error;
/// Built-in serializer implementations registered by the extension.
pub mod builtin;
mod call_scope;
mod config_adapter;
mod default_setup;
mod delegator;
mod error;
mod error_event;
mod extension;
mod extension_shared;
mod not_serializable_error;
mod serialization_extension_id;
mod serialization_extension_installer;
/// Serialization registry for type-to-serializer mappings.
pub mod serialization_registry;
mod serialization_setup;
mod serialized_message;
mod serializer;
mod serializer_id;
mod serializer_id_error;
mod string_manifest_serializer;
mod transport_information;

pub use builder::SerializationSetupBuilder;
pub use builder_error::SerializationBuilderError;
pub use call_scope::SerializationCallScope;
pub use config_adapter::SerializationConfigAdapter;
pub use default_setup::{default_serialization_extension_id, default_serialization_setup};
pub use delegator::SerializationDelegator;
pub use error::SerializationError;
pub use error_event::SerializationErrorEvent;
pub use extension::{SerializationExtension, SerializationExtensionGeneric};
pub use extension_shared::{SerializationExtensionShared, SerializationExtensionSharedGeneric};
pub use not_serializable_error::NotSerializableError;
pub use serialization_extension_id::SerializationExtensionId;
pub use serialization_extension_installer::SerializationExtensionInstaller;
pub use serialization_setup::SerializationSetup;
pub use serialized_message::SerializedMessage;
pub use serializer::Serializer;
pub use serializer_id::SerializerId;
pub use serializer_id_error::SerializerIdError;
pub use string_manifest_serializer::SerializerWithStringManifest;
pub use transport_information::TransportInformation;
