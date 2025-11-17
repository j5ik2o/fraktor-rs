//! Built-in serializer for `i32`.

use alloc::{boxed::Box, vec::Vec};
use core::any::Any;

use crate::core::serialization::{error::SerializationError, serializer::Serializer, serializer_id::SerializerId};

/// Serializes 32-bit signed integers.
pub struct I32Serializer {
  id: SerializerId,
}

impl I32Serializer {
  /// Creates a new instance with the provided identifier.
  #[must_use]
  pub const fn new(id: SerializerId) -> Self {
    Self { id }
  }
}

impl Serializer for I32Serializer {
  fn identifier(&self) -> SerializerId {
    self.id
  }

  fn to_binary(&self, message: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, SerializationError> {
    let value = message.downcast_ref::<i32>().ok_or(SerializationError::InvalidFormat)?;
    Ok(value.to_le_bytes().to_vec())
  }

  fn from_binary(
    &self,
    bytes: &[u8],
    _type_hint: Option<core::any::TypeId>,
  ) -> Result<Box<dyn Any + Send>, SerializationError> {
    if bytes.len() < core::mem::size_of::<i32>() {
      return Err(SerializationError::InvalidFormat);
    }
    let mut array = [0_u8; core::mem::size_of::<i32>()];
    array.copy_from_slice(&bytes[..core::mem::size_of::<i32>()]);
    Ok(Box::new(i32::from_le_bytes(array)))
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }
}
