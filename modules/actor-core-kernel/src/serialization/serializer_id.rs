//! Serializer identifier newtype.

#[cfg(test)]
#[path = "serializer_id_test.rs"]
mod tests;

use core::fmt::{self, Formatter, Result as FmtResult};

use super::serializer_id_error::SerializerIdError;

/// Unique identifier assigned to a serializer implementation.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SerializerId(u32);

impl SerializerId {
  /// Returns the underlying numeric identifier.
  #[must_use]
  pub const fn value(self) -> u32 {
    self.0
  }

  /// Creates a serializer id without performing validation.
  #[must_use]
  pub const fn from_raw(value: u32) -> Self {
    Self(value)
  }
}

impl fmt::Debug for SerializerId {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    f.debug_tuple("SerializerId").field(&self.0).finish()
  }
}

impl TryFrom<u32> for SerializerId {
  type Error = SerializerIdError;

  fn try_from(value: u32) -> Result<Self, Self::Error> {
    if value <= 40 {
      return Err(SerializerIdError::Reserved(value));
    }
    Ok(Self(value))
  }
}
