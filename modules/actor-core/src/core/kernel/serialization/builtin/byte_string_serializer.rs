//! Built-in serializer for [`ByteString`].

#[cfg(test)]
mod tests;

use alloc::{boxed::Box, vec::Vec};
use core::any::{Any, TypeId};

use crate::core::kernel::{
  serialization::{error::SerializationError, serializer::Serializer, serializer_id::SerializerId},
  support::ByteString,
};

/// Serializes [`ByteString`] values by copying the underlying byte payload.
pub struct ByteStringSerializer {
  id: SerializerId,
}

impl ByteStringSerializer {
  /// Creates a new instance with the provided identifier.
  #[must_use]
  pub const fn new(id: SerializerId) -> Self {
    Self { id }
  }
}

impl Serializer for ByteStringSerializer {
  fn identifier(&self) -> SerializerId {
    self.id
  }

  fn to_binary(&self, message: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, SerializationError> {
    let value = message.downcast_ref::<ByteString>().ok_or(SerializationError::InvalidFormat)?;
    Ok(value.to_vec())
  }

  fn from_binary(
    &self,
    bytes: &[u8],
    _type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    Ok(Box::new(ByteString::from_slice(bytes)))
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }
}
