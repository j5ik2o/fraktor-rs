//! Errors raised when constructing serializer identifiers.

/// Errors raised when constructing a [`SerializerId`](super::SerializerId).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SerializerIdError {
  /// Indicates that the identifier collides with the runtime reserved range.
  Reserved(u32),
}
