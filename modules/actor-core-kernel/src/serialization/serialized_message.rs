//! Serialized message container compatible with Pekko layout.

#[cfg(test)]
#[path = "serialized_message_test.rs"]
mod tests;

use alloc::{borrow::ToOwned, string::String, vec::Vec};

use super::{error::SerializationError, serializer_id::SerializerId};

/// Opaque serialized payload along with manifest metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerializedMessage {
  serializer_id: SerializerId,
  manifest:      Option<String>,
  bytes:         Vec<u8>,
}

impl SerializedMessage {
  fn end_offset(bytes: &[u8], cursor: usize, len: usize) -> Result<usize, SerializationError> {
    let end = cursor.checked_add(len).ok_or(SerializationError::InvalidFormat)?;
    if bytes.len() < end {
      return Err(SerializationError::InvalidFormat);
    }
    Ok(end)
  }

  fn read_u32_at(bytes: &[u8], cursor: usize) -> Result<(u32, usize), SerializationError> {
    let end = Self::end_offset(bytes, cursor, 4)?;
    let mut raw = [0_u8; 4];
    raw.copy_from_slice(&bytes[cursor..end]);
    Ok((u32::from_le_bytes(raw), end))
  }

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
    let (serializer_raw, serializer_end) = Self::read_u32_at(bytes, cursor)?;
    cursor = serializer_end;
    let serializer_id = SerializerId::from_raw(serializer_raw);
    let has_manifest = bytes[cursor];
    cursor += 1;
    let manifest = if has_manifest == 1 {
      let (manifest_len, len_end) = Self::read_u32_at(bytes, cursor)?;
      let manifest_len = manifest_len as usize;
      cursor = len_end;
      let manifest_end = Self::end_offset(bytes, cursor, manifest_len)?;
      let manifest_bytes = &bytes[cursor..manifest_end];
      cursor = manifest_end;
      let manifest_str = core::str::from_utf8(manifest_bytes).map_err(|_| SerializationError::InvalidFormat)?;
      Some(manifest_str.to_owned())
    } else if has_manifest == 0 {
      None
    } else {
      return Err(SerializationError::InvalidFormat);
    };
    let (payload_len_u32, len_end) = Self::read_u32_at(bytes, cursor)?;
    let payload_len = payload_len_u32 as usize;
    cursor = len_end;
    let payload_end = Self::end_offset(bytes, cursor, payload_len)?;
    if payload_end != bytes.len() {
      return Err(SerializationError::InvalidFormat);
    }
    let payload = bytes[cursor..payload_end].to_vec();
    Ok(Self::new(serializer_id, manifest, payload))
  }
}
