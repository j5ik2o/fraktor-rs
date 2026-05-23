use alloc::vec::Vec;

use fraktor_actor_core_kernel_rs::serialization::SerializationError;

use super::{ensure_finished, read_bool, read_bytes, read_string, read_u8, read_u32, read_u64, write_bytes, write_u32};

#[test]
fn read_bytes_rejects_declared_length_beyond_remaining_bytes() {
  let mut bytes = Vec::new();
  write_u32(&mut bytes, 4);
  bytes.extend_from_slice(&[1, 2]);
  let mut cursor = 0;

  let result = read_bytes(&bytes, &mut cursor);

  assert_eq!(result, Err(SerializationError::InvalidFormat));
}

#[test]
fn primitive_reads_reject_truncated_input() {
  let mut cursor = 0;
  assert_eq!(read_u8(&[], &mut cursor), Err(SerializationError::InvalidFormat));

  let mut cursor = 0;
  assert_eq!(read_u32(&[1, 2, 3], &mut cursor), Err(SerializationError::InvalidFormat));

  let mut cursor = 0;
  assert_eq!(read_u64(&[1, 2, 3, 4, 5, 6, 7], &mut cursor), Err(SerializationError::InvalidFormat));
}

#[test]
fn primitive_reads_reject_cursor_overflow() {
  let mut cursor = usize::MAX;
  assert_eq!(read_u8(&[1], &mut cursor), Err(SerializationError::InvalidFormat));

  let mut cursor = usize::MAX;
  assert_eq!(read_u32(&[1, 2, 3, 4], &mut cursor), Err(SerializationError::InvalidFormat));

  let mut cursor = usize::MAX;
  assert_eq!(read_u64(&[1, 2, 3, 4, 5, 6, 7, 8], &mut cursor), Err(SerializationError::InvalidFormat));
}

#[test]
fn read_bool_rejects_unknown_discriminant() {
  let mut cursor = 0;

  assert_eq!(read_bool(&[2], &mut cursor), Err(SerializationError::InvalidFormat));
}

#[test]
fn read_string_rejects_invalid_utf8() {
  let mut bytes = Vec::new();
  write_bytes(&mut bytes, &[0xff]).expect("bytes");
  let mut cursor = 0;

  assert_eq!(read_string(&bytes, &mut cursor), Err(SerializationError::InvalidFormat));
}

#[test]
fn ensure_finished_rejects_trailing_bytes() {
  assert_eq!(ensure_finished(&[1, 2], 1), Err(SerializationError::InvalidFormat));
}
