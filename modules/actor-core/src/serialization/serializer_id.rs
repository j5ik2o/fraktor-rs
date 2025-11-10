//! Serializer identifier newtype.

#[cfg(test)]
mod tests;

use core::fmt;

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
  pub(crate) const fn from_raw(value: u32) -> Self {
    Self(value)
  }
}

impl fmt::Debug for SerializerId {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_tuple("SerializerId").field(&self.0).finish()
  }
}

/// Errors raised when constructing a [`SerializerId`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SerializerIdError {
  /// Indicates that the identifier collides with the runtime reserved range.
  Reserved(u32),
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
