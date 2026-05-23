//! Internal persistence serialization wire helpers.

#[cfg(test)]
#[path = "wire_test.rs"]
mod tests;

use alloc::{
  string::{String, ToString},
  vec::Vec,
};

use fraktor_actor_core_kernel_rs::serialization::{SerializationError, SerializedMessage};

pub(crate) const PERSISTENT_REPR_TAG: u8 = 1;
pub(crate) const ATOMIC_WRITE_TAG: u8 = 2;

pub(crate) fn write_u8(buffer: &mut Vec<u8>, value: u8) {
  buffer.push(value);
}

pub(crate) fn write_bool(buffer: &mut Vec<u8>, value: bool) {
  buffer.push(u8::from(value));
}

pub(crate) fn write_u32(buffer: &mut Vec<u8>, value: u32) {
  buffer.extend_from_slice(&value.to_le_bytes());
}

pub(crate) fn write_u64(buffer: &mut Vec<u8>, value: u64) {
  buffer.extend_from_slice(&value.to_le_bytes());
}

pub(crate) fn write_bytes(buffer: &mut Vec<u8>, bytes: &[u8]) -> Result<(), SerializationError> {
  let len = u32::try_from(bytes.len()).map_err(|_| SerializationError::InvalidFormat)?;
  write_u32(buffer, len);
  buffer.extend_from_slice(bytes);
  Ok(())
}

pub(crate) fn write_string(buffer: &mut Vec<u8>, value: &str) -> Result<(), SerializationError> {
  write_bytes(buffer, value.as_bytes())
}

pub(crate) fn write_serialized(buffer: &mut Vec<u8>, value: &SerializedMessage) -> Result<(), SerializationError> {
  write_bytes(buffer, &value.encode())
}

pub(crate) fn read_u8(bytes: &[u8], cursor: &mut usize) -> Result<u8, SerializationError> {
  let end = cursor.checked_add(1).ok_or(SerializationError::InvalidFormat)?;
  if bytes.len() < end {
    return Err(SerializationError::InvalidFormat);
  }
  let value = bytes[*cursor];
  *cursor = end;
  Ok(value)
}

pub(crate) fn read_bool(bytes: &[u8], cursor: &mut usize) -> Result<bool, SerializationError> {
  match read_u8(bytes, cursor)? {
    | 0 => Ok(false),
    | 1 => Ok(true),
    | _ => Err(SerializationError::InvalidFormat),
  }
}

pub(crate) fn read_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, SerializationError> {
  let end = cursor.checked_add(4).ok_or(SerializationError::InvalidFormat)?;
  if bytes.len() < end {
    return Err(SerializationError::InvalidFormat);
  }
  let mut array = [0_u8; 4];
  array.copy_from_slice(&bytes[*cursor..end]);
  *cursor = end;
  Ok(u32::from_le_bytes(array))
}

pub(crate) fn read_u64(bytes: &[u8], cursor: &mut usize) -> Result<u64, SerializationError> {
  let end = cursor.checked_add(8).ok_or(SerializationError::InvalidFormat)?;
  if bytes.len() < end {
    return Err(SerializationError::InvalidFormat);
  }
  let mut array = [0_u8; 8];
  array.copy_from_slice(&bytes[*cursor..end]);
  *cursor = end;
  Ok(u64::from_le_bytes(array))
}

pub(crate) fn read_bytes<'a>(bytes: &'a [u8], cursor: &mut usize) -> Result<&'a [u8], SerializationError> {
  let len = read_u32(bytes, cursor)? as usize;
  let end = cursor.checked_add(len).ok_or(SerializationError::InvalidFormat)?;
  if end > bytes.len() {
    return Err(SerializationError::InvalidFormat);
  }
  let result = &bytes[*cursor..end];
  *cursor = end;
  Ok(result)
}

pub(crate) fn read_string(bytes: &[u8], cursor: &mut usize) -> Result<String, SerializationError> {
  let bytes = read_bytes(bytes, cursor)?;
  let value = core::str::from_utf8(bytes).map_err(|_| SerializationError::InvalidFormat)?;
  Ok(value.to_string())
}

pub(crate) fn read_serialized(bytes: &[u8], cursor: &mut usize) -> Result<SerializedMessage, SerializationError> {
  SerializedMessage::decode(read_bytes(bytes, cursor)?)
}

pub(crate) const fn ensure_finished(bytes: &[u8], cursor: usize) -> Result<(), SerializationError> {
  if cursor == bytes.len() { Ok(()) } else { Err(SerializationError::InvalidFormat) }
}
