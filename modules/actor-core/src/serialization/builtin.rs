//! Built-in serializer implementations registered by the extension.

pub mod bool_serializer;
pub mod bytes_serializer;
pub mod i32_serializer;
pub mod null_serializer;
pub mod string_serializer;

use alloc::string::String;

pub use bool_serializer::BoolSerializer;
pub use bytes_serializer::BytesSerializer;
use cellactor_utils_core_rs::sync::ArcShared;
pub use i32_serializer::I32Serializer;
pub use null_serializer::NullSerializer;
pub use string_serializer::StringSerializer;

use crate::{
  RuntimeToolbox,
  serialization::{
    error::SerializationError, serialization_registry::SerializationRegistryGeneric, serializer::Serializer,
    serializer_id::SerializerId,
  },
};

const NULL_ID: SerializerId = SerializerId::from_raw(1);
const BOOL_ID: SerializerId = SerializerId::from_raw(2);
const I32_ID: SerializerId = SerializerId::from_raw(3);
const STRING_ID: SerializerId = SerializerId::from_raw(4);
const BYTES_ID: SerializerId = SerializerId::from_raw(5);

/// Registers built-in serializers required by the runtime.
pub fn register_defaults<TB: RuntimeToolbox + 'static, F>(
  registry: &SerializationRegistryGeneric<TB>,
  mut on_collision: F,
) -> Result<(), SerializationError>
where
  F: FnMut(&'static str, SerializerId), {
  register::<TB, _, _>(
    registry,
    NULL_ID,
    NullSerializer::new(NULL_ID),
    "null",
    Some((core::any::TypeId::of::<()>(), "()".into())),
    &mut on_collision,
  )?;
  register::<TB, _, _>(
    registry,
    BOOL_ID,
    BoolSerializer::new(BOOL_ID),
    "bool",
    Some((core::any::TypeId::of::<bool>(), "bool".into())),
    &mut on_collision,
  )?;
  register::<TB, _, _>(
    registry,
    I32_ID,
    I32Serializer::new(I32_ID),
    "i32",
    Some((core::any::TypeId::of::<i32>(), "i32".into())),
    &mut on_collision,
  )?;
  register::<TB, _, _>(
    registry,
    STRING_ID,
    StringSerializer::new(STRING_ID),
    "string",
    Some((core::any::TypeId::of::<alloc::string::String>(), "String".into())),
    &mut on_collision,
  )?;
  register::<TB, _, _>(
    registry,
    BYTES_ID,
    BytesSerializer::new(BYTES_ID),
    "bytes",
    Some((core::any::TypeId::of::<alloc::vec::Vec<u8>>(), "Vec<u8>".into())),
    &mut on_collision,
  )?;
  Ok(())
}

fn register<TB: RuntimeToolbox + 'static, S, F>(
  registry: &SerializationRegistryGeneric<TB>,
  id: SerializerId,
  serializer: S,
  name: &'static str,
  binding: Option<(core::any::TypeId, String)>,
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
