//! Serialized message container compatible with Pekko layout.

#[cfg(test)]
mod tests;

use alloc::{borrow::ToOwned, string::String, vec::Vec};
use core::convert::TryInto;

use super::{error::SerializationError, serializer_id::SerializerId};

/// Opaque serialized payload along with manifest metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerializedMessage {
  serializer_id: SerializerId,
  manifest:      Option<String>,
  bytes:         Vec<u8>,
}

impl SerializedMessage {
  /// Creates a new serialized message.
  #[must_use]
  pub const fn new(serializer_id: SerializerId, manifest: Option<String>, bytes: Vec<u8>) -> Self {
    Self { serializer_id, manifest, bytes }
  }

  /// Returns the serializer identifier.
  #[must_use]
  pub const fn serializer_id(&self) -> SerializerId {
    self.serializer_id
  }

  /// Returns the optional manifest string.
  #[must_use]
  pub fn manifest(&self) -> Option<&str> {
    self.manifest.as_deref()
  }

  /// Returns the raw byte payload.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)] // Vec の Deref が const でないため const fn にできない
  pub fn bytes(&self) -> &[u8] {
    &self.bytes
  }

  /// Encodes the message into the Pekko-compatible byte layout.
  ///
  /// # Panics
  ///
  /// This function does not panic under normal circumstances.
  #[must_use]
  pub fn encode(&self) -> Vec<u8> {
    let manifest_bytes = self.manifest.as_ref().map(String::as_bytes);
    let manifest_len = manifest_bytes.map_or(0, |bytes| bytes.len());
    let mut buffer =
      Vec::with_capacity(4 + 1 + manifest_len + 4 + self.bytes.len() + if manifest_bytes.is_some() { 4 } else { 0 });
    buffer.extend_from_slice(&self.serializer_id.value().to_le_bytes());
    match manifest_bytes {
      | Some(bytes) => {
        buffer.push(1);
        buffer.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        buffer.extend_from_slice(bytes);
      },
      | None => buffer.push(0),
    }
    buffer.extend_from_slice(&(self.bytes.len() as u32).to_le_bytes());
    buffer.extend_from_slice(&self.bytes);
    buffer
  }

  /// Decodes the message from the provided byte slice.
  ///
  /// # Errors
  ///
  /// Returns [`SerializationError::InvalidFormat`] when the bytes do not follow the expected
  /// layout.
  pub fn decode(bytes: &[u8]) -> Result<Self, SerializationError> {
    let mut cursor = 0;
    if bytes.len() < 5 {
      return Err(SerializationError::InvalidFormat);
    }
    let serializer_raw =
      u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().map_err(|_| SerializationError::InvalidFormat)?);
    cursor += 4;
    let serializer_id = SerializerId::from_raw(serializer_raw);
    let has_manifest = bytes[cursor];
    cursor += 1;
    let manifest = if has_manifest == 1 {
      if bytes.len() < cursor + 4 {
        return Err(SerializationError::InvalidFormat);
      }
      let manifest_len =
        u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().map_err(|_| SerializationError::InvalidFormat)?)
          as usize;
      cursor += 4;
      if bytes.len() < cursor + manifest_len {
        return Err(SerializationError::InvalidFormat);
      }
      let manifest_bytes = &bytes[cursor..cursor + manifest_len];
      cursor += manifest_len;
      let manifest_str = core::str::from_utf8(manifest_bytes).map_err(|_| SerializationError::InvalidFormat)?;
      Some(manifest_str.to_owned())
    } else if has_manifest == 0 {
      None
    } else {
      return Err(SerializationError::InvalidFormat);
    };
    if bytes.len() < cursor + 4 {
      return Err(SerializationError::InvalidFormat);
    }
    let payload_len =
      u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().map_err(|_| SerializationError::InvalidFormat)?) as usize;
    cursor += 4;
    if bytes.len() < cursor + payload_len {
      return Err(SerializationError::InvalidFormat);
    }
    let payload = bytes[cursor..cursor + payload_len].to_vec();
    Ok(Self::new(serializer_id, manifest, payload))
  }
}
