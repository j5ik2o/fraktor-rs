use alloc::vec::Vec;

use fraktor_actor_core_kernel_rs::serialization::SerializationError;

use super::{read_bytes, write_u32};

#[test]
fn read_bytes_rejects_declared_length_beyond_remaining_bytes() {
  let mut bytes = Vec::new();
  write_u32(&mut bytes, 4);
  bytes.extend_from_slice(&[1, 2]);
  let mut cursor = 0;

  let result = read_bytes(&bytes, &mut cursor);

  assert_eq!(result, Err(SerializationError::InvalidFormat));
}
