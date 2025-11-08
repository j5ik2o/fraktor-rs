use alloc::{boxed::Box, string::ToString};
use core::any::Any;

use bincode::config::Config;
use erased_serde::Serialize as ErasedSerialize;

use super::{bytes::Bytes, error::SerializationError, serializer::SerializerImpl};

#[cfg(test)]
mod tests;

/// Default serializer backed by `bincode`.
#[derive(Default, Clone)]
pub struct BincodeSerializer;

impl BincodeSerializer {
  /// Creates a new serializer instance.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }

  fn config() -> impl Config {
    bincode::config::standard().with_fixed_int_encoding()
  }
}

impl SerializerImpl for BincodeSerializer {
  fn identifier(&self) -> u32 {
    1
  }

  fn serialize_erased(&self, value: &dyn ErasedSerialize) -> Result<Bytes, SerializationError> {
    bincode::serde::encode_to_vec(value, Self::config())
      .map(Bytes::from_vec)
      .map_err(|error| SerializationError::SerializationFailed(error.to_string()))
  }

  fn deserialize(&self, _bytes: &[u8], manifest: &str) -> Result<Box<dyn Any + Send>, SerializationError> {
    Err(SerializationError::UnknownManifest { serializer_id: self.identifier(), manifest: manifest.to_string() })
  }
}
