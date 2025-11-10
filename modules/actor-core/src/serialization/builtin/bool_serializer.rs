//! Built-in serializer for `bool`.

use alloc::{boxed::Box, vec, vec::Vec};
use core::any::Any;

use crate::serialization::{error::SerializationError, serializer::Serializer, serializer_id::SerializerId};

/// Serializes boolean values as a single byte.
pub struct BoolSerializer {
  id: SerializerId,
}

impl BoolSerializer {
  /// Creates a new instance with the provided identifier.
  #[must_use]
  pub const fn new(id: SerializerId) -> Self {
    Self { id }
  }
}

impl Serializer for BoolSerializer {
  fn identifier(&self) -> SerializerId {
    self.id
  }

  fn to_binary(&self, message: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, SerializationError> {
    let value = message.downcast_ref::<bool>().ok_or(SerializationError::InvalidFormat)?;
    Ok(vec![u8::from(*value)])
  }

  fn from_binary(
    &self,
    bytes: &[u8],
    _type_hint: Option<core::any::TypeId>,
  ) -> Result<Box<dyn Any + Send>, SerializationError> {
    let first = bytes.first().ok_or(SerializationError::InvalidFormat)?;
    Ok(Box::new(*first != 0))
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }
}
