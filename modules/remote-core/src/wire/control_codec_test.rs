use bytes::Bytes;

use super::{compression_entry_count, decode_compression_entries};
use crate::wire::WireError;

#[test]
fn compression_entry_count_rejects_lengths_over_u32_max() {
  let err = compression_entry_count(u32::MAX as usize + 1).unwrap_err();

  assert_eq!(err, WireError::InvalidFormat);
}

#[test]
fn decode_compression_entries_returns_truncated_when_later_entry_id_is_missing() {
  let mut buf = Bytes::from_static(&[
    0x00, 0x00, 0x00, 0x02, // entry_count = 2
    0x00, 0x00, 0x00, 0x01, // first entry id
    0x00, 0x00, 0x00, 0x08, // first entry literal length = 8
    b'a', b'b', b'c', b'd', b'e', b'f', b'g', b'h',
  ]);

  let err = decode_compression_entries(&mut buf).unwrap_err();

  assert_eq!(err, WireError::Truncated);
}
