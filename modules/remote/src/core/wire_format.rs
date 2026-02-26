use alloc::{string::String, vec::Vec};
use core::convert::TryInto;

use crate::core::wire_error::WireError;

/// Writes a length-prefixed UTF-8 string into the buffer.
pub(crate) fn write_string(buffer: &mut Vec<u8>, value: &str) {
  let bytes = value.as_bytes();
  let len = u32::try_from(bytes.len()).expect("string length must fit in u32");
  buffer.extend_from_slice(&len.to_le_bytes());
  buffer.extend_from_slice(bytes);
}

/// Reads a length-prefixed UTF-8 string from the byte slice at the given cursor position.
///
/// # Errors
///
/// Returns [`WireError`] when the buffer is too short or the bytes are not valid UTF-8.
pub(crate) fn read_string(bytes: &[u8], cursor: &mut usize) -> Result<String, WireError> {
  let len_end = (*cursor).checked_add(4).ok_or(WireError::InvalidFormat)?;
  if bytes.len() < len_end {
    return Err(WireError::InvalidFormat);
  }
  let len = u32::from_le_bytes(bytes[*cursor..len_end].try_into().map_err(|_| WireError::InvalidFormat)?) as usize;
  *cursor = len_end;
  let str_end = (*cursor).checked_add(len).ok_or(WireError::InvalidFormat)?;
  if bytes.len() < str_end {
    return Err(WireError::InvalidFormat);
  }
  let slice = &bytes[*cursor..str_end];
  *cursor = str_end;
  Ok(String::from_utf8(slice.to_vec())?)
}

/// Writes a single boolean byte (1 for true, 0 for false) into the buffer.
pub(crate) fn write_bool(buffer: &mut Vec<u8>, value: bool) {
  buffer.push(u8::from(value));
}

/// Reads a single boolean byte (0 or 1) from the byte slice at the given cursor position.
///
/// # Errors
///
/// Returns [`WireError`] when the buffer is too short or the byte is not 0 or 1.
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
