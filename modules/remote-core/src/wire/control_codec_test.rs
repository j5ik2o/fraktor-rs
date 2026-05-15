use super::compression_entry_count;
use crate::wire::WireError;

#[test]
fn compression_entry_count_rejects_lengths_over_u32_max() {
  let err = compression_entry_count(u32::MAX as usize + 1).unwrap_err();

  assert_eq!(err, WireError::InvalidFormat);
}
