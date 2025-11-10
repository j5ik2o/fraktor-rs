//! Serialization subsystem aggregation.

pub mod builder;
pub mod builder_error;
pub mod builtin;
pub mod call_scope;
pub mod config_adapter;
pub mod delegator;
pub mod error;
pub mod error_event;
pub mod extension;
pub mod not_serializable_error;
pub mod serialization_registry;
pub mod serialization_setup;
pub mod serialized_message;
pub mod serializer;
pub mod serializer_id;
pub mod string_manifest_serializer;
pub mod transport_information;
