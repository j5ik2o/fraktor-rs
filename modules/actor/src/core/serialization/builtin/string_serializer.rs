//! Built-in serializer for `String`.

use alloc::{boxed::Box, string::String, vec::Vec};
use core::any::Any;

use crate::core::serialization::{error::SerializationError, serializer::Serializer, serializer_id::SerializerId};

/// Serializes UTF-8 strings with a length prefix.
pub struct StringSerializer {
  id: SerializerId,
}

impl StringSerializer {
  /// Creates a new instance with the provided identifier.
  #[must_use]
  pub const fn new(id: SerializerId) -> Self {
    Self { id }
  }
}

impl Serializer for StringSerializer {
  fn identifier(&self) -> SerializerId {
    self.id
  }

  fn to_binary(&self, message: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, SerializationError> {
    let value = message.downcast_ref::<String>().ok_or(SerializationError::InvalidFormat)?;
    let bytes = value.as_bytes();
    let mut buffer = Vec::with_capacity(4 + bytes.len());
    buffer.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    buffer.extend_from_slice(bytes);
    Ok(buffer)
  }

  fn from_binary(
    &self,
    bytes: &[u8],
    _type_hint: Option<core::any::TypeId>,
  ) -> Result<Box<dyn Any + Send>, SerializationError> {
    if bytes.len() < 4 {
      return Err(SerializationError::InvalidFormat);
    }
    let len = u32::from_le_bytes(bytes[0..4].try_into().map_err(|_| SerializationError::InvalidFormat)?) as usize;
    if bytes.len() < 4 + len {
      return Err(SerializationError::InvalidFormat);
    }
    let payload = core::str::from_utf8(&bytes[4..4 + len]).map_err(|_| SerializationError::InvalidFormat)?;
    Ok(Box::new(String::from(payload)))
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }
}
