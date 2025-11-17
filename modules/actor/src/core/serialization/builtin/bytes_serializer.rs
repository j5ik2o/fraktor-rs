//! Built-in serializer for `Vec<u8>`.

use alloc::{boxed::Box, vec::Vec};
use core::any::Any;

use crate::core::serialization::{error::SerializationError, serializer::Serializer, serializer_id::SerializerId};

/// Serializes byte buffers by cloning the payload.
pub struct BytesSerializer {
  id: SerializerId,
}

impl BytesSerializer {
  /// Creates a new instance with the provided identifier.
  #[must_use]
  pub const fn new(id: SerializerId) -> Self {
    Self { id }
  }
}

impl Serializer for BytesSerializer {
  fn identifier(&self) -> SerializerId {
    self.id
  }

  fn to_binary(&self, message: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, SerializationError> {
    let value = message.downcast_ref::<Vec<u8>>().ok_or(SerializationError::InvalidFormat)?;
    Ok(value.clone())
  }

  fn from_binary(
    &self,
    bytes: &[u8],
    _type_hint: Option<core::any::TypeId>,
  ) -> Result<Box<dyn Any + Send>, SerializationError> {
    Ok(Box::new(bytes.to_vec()))
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }
}
