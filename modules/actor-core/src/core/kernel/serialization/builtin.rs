//! Built-in serializer implementations registered by the extension.

mod bool_serializer;
mod byte_string_serializer;
mod bytes_serializer;
mod i32_serializer;
mod message_container_serializer;
mod null_serializer;
mod string_serializer;
mod system_message_serializer;

use alloc::{string::String, vec::Vec};
use core::any::TypeId;

pub use bool_serializer::BoolSerializer;
pub use byte_string_serializer::ByteStringSerializer;
pub use bytes_serializer::BytesSerializer;
use fraktor_utils_core_rs::core::sync::ArcShared;
pub use i32_serializer::I32Serializer;
pub use message_container_serializer::MessageContainerSerializer;
pub use null_serializer::NullSerializer;
pub use string_serializer::StringSerializer;
pub use system_message_serializer::SystemMessageSerializer;

use crate::core::kernel::{
  actor::{
    actor_selection::ActorSelectionMessage,
    messaging::system_message::SystemMessage,
  },
  serialization::{
    error::SerializationError, serialization_registry::SerializationRegistry, serializer::Serializer,
    serializer_id::SerializerId,
  },
  util::ByteString,
};

/// Serializer ID for null/unit type.
pub const NULL_ID: SerializerId = SerializerId::from_raw(1);

/// Serializer ID for boolean type.
pub const BOOL_ID: SerializerId = SerializerId::from_raw(2);

/// Serializer ID for i32 integer type.
pub const I32_ID: SerializerId = SerializerId::from_raw(3);

/// Serializer ID for string type.
pub const STRING_ID: SerializerId = SerializerId::from_raw(4);

/// Serializer ID for byte array type.
pub const BYTES_ID: SerializerId = SerializerId::from_raw(5);

/// Serializer ID for [`ByteString`](crate::core::kernel::util::ByteString) type.
pub const BYTE_STRING_ID: SerializerId = SerializerId::from_raw(6);

/// Serializer ID for [`SystemMessage`].
pub const SYSTEM_MESSAGE_ID: SerializerId = SerializerId::from_raw(7);

/// Serializer ID for [`ActorSelectionMessage`].
pub const MESSAGE_CONTAINER_ID: SerializerId = SerializerId::from_raw(8);

/// Registers built-in serializers required by the runtime.
///
/// # Errors
///
/// Returns `SerializationError` if type binding registration fails during the process.
pub fn register_defaults<F>(
  registry: &ArcShared<SerializationRegistry>,
  mut on_collision: F,
) -> Result<(), SerializationError>
where
  F: FnMut(&'static str, SerializerId), {
  register::<_, _>(
    registry,
    NULL_ID,
    NullSerializer::new(NULL_ID),
    "null",
    Some((TypeId::of::<()>(), "()".into())),
    &mut on_collision,
  )?;
  register::<_, _>(
    registry,
    BOOL_ID,
    BoolSerializer::new(BOOL_ID),
    "bool",
    Some((TypeId::of::<bool>(), "bool".into())),
    &mut on_collision,
  )?;
  register::<_, _>(
    registry,
    I32_ID,
    I32Serializer::new(I32_ID),
    "i32",
    Some((TypeId::of::<i32>(), "i32".into())),
    &mut on_collision,
  )?;
  register::<_, _>(
    registry,
    STRING_ID,
    StringSerializer::new(STRING_ID),
    "string",
    Some((TypeId::of::<String>(), "String".into())),
    &mut on_collision,
  )?;
  register::<_, _>(
    registry,
    BYTES_ID,
    BytesSerializer::new(BYTES_ID),
    "bytes",
    Some((TypeId::of::<Vec<u8>>(), "Vec<u8>".into())),
    &mut on_collision,
  )?;
  register::<_, _>(
    registry,
    BYTE_STRING_ID,
    ByteStringSerializer::new(BYTE_STRING_ID),
    "byte_string",
    Some((TypeId::of::<ByteString>(), "ByteString".into())),
    &mut on_collision,
  )?;
  register::<_, _>(
    registry,
    SYSTEM_MESSAGE_ID,
    SystemMessageSerializer::new(SYSTEM_MESSAGE_ID),
    "system_message",
    Some((TypeId::of::<SystemMessage>(), "SystemMessage".into())),
    &mut on_collision,
  )?;
  register::<_, _>(
    registry,
    MESSAGE_CONTAINER_ID,
    MessageContainerSerializer::new(MESSAGE_CONTAINER_ID, registry.downgrade()),
    "message_container",
    Some((TypeId::of::<ActorSelectionMessage>(), "ActorSelectionMessage".into())),
    &mut on_collision,
  )?;
  Ok(())
}

fn register<S, F>(
  registry: &SerializationRegistry,
  id: SerializerId,
  serializer: S,
  name: &'static str,
  binding: Option<(TypeId, String)>,
  on_collision: &mut F,
) -> Result<(), SerializationError>
where
  S: Serializer + 'static,
  F: FnMut(&'static str, SerializerId), {
  if !registry.register_serializer(id, ArcShared::new(serializer)) {
    on_collision(name, id);
    return Ok(());
  }
  if let Some((type_id, type_name)) = binding {
    registry.register_binding(type_id, type_name, id)?;
  }
  Ok(())
}
