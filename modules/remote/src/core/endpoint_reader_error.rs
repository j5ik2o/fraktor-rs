//! Error variants emitted when decoding inbound transport frames.

use fraktor_actor_rs::core::serialization::SerializationError;

/// Represents failures that can occur while decoding inbound envelopes.
#[derive(Debug)]
pub enum EndpointReaderError {
  /// The payload could not be deserialized into a runtime message.
  Deserialization(SerializationError),
}

impl core::fmt::Display for EndpointReaderError {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      | Self::Deserialization(error) => write!(f, "deserialization failed: {error:?}"),
    }
  }
}

impl From<SerializationError> for EndpointReaderError {
  fn from(value: SerializationError) -> Self {
    Self::Deserialization(value)
  }
}
