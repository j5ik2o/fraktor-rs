use alloc::format;

use super::SerializationError;

#[test]
fn invalid_format_debug_representation() {
  let error = SerializationError::InvalidFormat;
  let debug = format!("{error:?}");
  assert!(debug.contains("InvalidFormat"));
}
