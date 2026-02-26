use alloc::{string::String, vec::Vec};
use core::convert::TryInto;

use crate::core::wire_error::WireError;

pub(crate) fn write_string(buffer: &mut Vec<u8>, value: &str) {
  let bytes = value.as_bytes();
  buffer.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
  buffer.extend_from_slice(bytes);
}

pub(crate) fn read_string(bytes: &[u8], cursor: &mut usize) -> Result<String, WireError> {
  if bytes.len() < *cursor + 4 {
    return Err(WireError::InvalidFormat);
  }
  let len = u32::from_le_bytes(bytes[*cursor..*cursor + 4].try_into().map_err(|_| WireError::InvalidFormat)?) as usize;
  *cursor += 4;
  if bytes.len() < *cursor + len {
    return Err(WireError::InvalidFormat);
  }
  let slice = &bytes[*cursor..*cursor + len];
  *cursor += len;
  Ok(String::from_utf8(slice.to_vec())?)
}

pub(crate) fn read_bool(bytes: &[u8], cursor: &mut usize) -> Result<bool, WireError> {
  if bytes.len() <= *cursor {
    return Err(WireError::InvalidFormat);
  }
  let value = bytes[*cursor];
  *cursor += 1;
  match value {
    | 0 => Ok(false),
    | 1 => Ok(true),
    | _ => Err(WireError::InvalidFormat),
  }
}
