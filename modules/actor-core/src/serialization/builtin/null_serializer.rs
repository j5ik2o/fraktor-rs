//! Built-in serializer for `()`.

use alloc::{boxed::Box, vec::Vec};
use core::any::Any;

use crate::serialization::{error::SerializationError, serializer::Serializer, serializer_id::SerializerId};

/// Serializes the unit type by producing an empty payload.
pub struct NullSerializer {
  id: SerializerId,
}

impl NullSerializer {
  /// Creates a new instance with the provided identifier.
  #[must_use]
  pub const fn new(id: SerializerId) -> Self {
    Self { id }
  }
}

impl Serializer for NullSerializer {
  fn identifier(&self) -> SerializerId {
    self.id
  }

  fn to_binary(&self, _message: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, SerializationError> {
    Ok(Vec::new())
  }

  fn from_binary(
    &self,
    _bytes: &[u8],
    _type_hint: Option<core::any::TypeId>,
  ) -> Result<Box<dyn Any + Send>, SerializationError> {
    Ok(Box::new(()))
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }
}
