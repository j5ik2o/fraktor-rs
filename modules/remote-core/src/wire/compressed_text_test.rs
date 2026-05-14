use alloc::string::ToString;

use bytes::{Bytes, BytesMut};

use super::{CompressedText, decode_compressed_text, decode_option_compressed_text, encode_compressed_text};
use crate::wire::WireError;

fn encode(value: &CompressedText) -> Bytes {
  let mut buf = BytesMut::new();
  encode_compressed_text(value, &mut buf).unwrap();
  buf.freeze()
}

#[test]
fn literal_roundtrips() {
  let mut bytes = encode(&CompressedText::literal("/user/a".to_string()));
  let decoded = decode_compressed_text(&mut bytes).unwrap();

  assert_eq!(decoded.as_literal(), Some("/user/a"));
  assert_eq!(bytes.len(), 0);
}

#[test]
fn table_ref_roundtrips() {
  let mut bytes = encode(&CompressedText::table_ref(7));
  let decoded = decode_compressed_text(&mut bytes).unwrap();

  assert_eq!(decoded.as_table_ref(), Some(7));
  assert_eq!(bytes.len(), 0);
}

#[test]
fn table_ref_is_not_literal() {
  assert_eq!(CompressedText::table_ref(7).as_literal(), None);
}

#[test]
fn literal_is_not_table_ref() {
  assert_eq!(CompressedText::literal("/user/a".to_string()).as_table_ref(), None);
}

#[test]
fn empty_input_is_rejected() {
  let mut bytes = Bytes::new();
  let err = decode_compressed_text(&mut bytes).unwrap_err();

  assert_eq!(err, WireError::Truncated);
}

#[test]
fn empty_option_input_is_rejected() {
  let mut bytes = Bytes::new();
  let err = decode_option_compressed_text(&mut bytes).unwrap_err();

  assert_eq!(err, WireError::Truncated);
}

#[test]
fn unknown_option_tag_is_rejected() {
  let mut bytes = Bytes::from_static(&[0xff]);
  let err = decode_option_compressed_text(&mut bytes).unwrap_err();

  assert_eq!(err, WireError::InvalidFormat);
}

#[test]
fn unknown_tag_is_rejected() {
  let mut bytes = Bytes::from_static(&[0xff]);
  let err = decode_compressed_text(&mut bytes).unwrap_err();

  assert_eq!(err, WireError::InvalidFormat);
}

#[test]
fn truncated_literal_is_rejected() {
  let mut bytes = Bytes::from_static(&[0x00, 0x00, 0x00]);
  let err = decode_compressed_text(&mut bytes).unwrap_err();

  assert_eq!(err, WireError::Truncated);
}

#[test]
fn truncated_table_ref_is_rejected() {
  let mut bytes = Bytes::from_static(&[0x01, 0x00, 0x00, 0x00]);
  let err = decode_compressed_text(&mut bytes).unwrap_err();

  assert_eq!(err, WireError::Truncated);
}
