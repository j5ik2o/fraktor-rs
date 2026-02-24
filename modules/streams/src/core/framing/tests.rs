use alloc::{vec, vec::Vec};

use super::{find_delimiter, read_big_endian_uint};

#[test]
fn should_find_delimiter_at_start() {
  assert_eq!(find_delimiter(b"abc", b"a"), Some(0));
}

#[test]
fn should_find_delimiter_in_middle() {
  assert_eq!(find_delimiter(b"hello\nworld", b"\n"), Some(5));
}

#[test]
fn should_return_none_when_delimiter_absent() {
  assert_eq!(find_delimiter(b"hello", b"\n"), None);
}

#[test]
fn should_return_none_for_empty_needle() {
  assert_eq!(find_delimiter(b"hello", b""), None);
}

#[test]
fn should_read_big_endian_u16() {
  assert_eq!(read_big_endian_uint(&[0x00, 0x0A]), 10);
}

#[test]
fn should_read_big_endian_u32() {
  assert_eq!(read_big_endian_uint(&[0x00, 0x00, 0x01, 0x00]), 256);
}

#[test]
fn should_read_single_byte() {
  assert_eq!(read_big_endian_uint(&[0x05]), 5);
}

#[test]
fn should_create_delimiter_flow() {
  use crate::core::stage::Source;

  let framing = super::Framing::delimiter(vec![b'\n'], 1024, false);
  let source = Source::from(vec![b"hello\nwor".to_vec(), b"ld\nfoo".to_vec()]);
  let result = source.via(framing).collect_values();
  let frames = result.unwrap();
  assert_eq!(frames, vec![b"hello".to_vec(), b"world".to_vec()]);
}

#[test]
fn should_emit_trailing_bytes_when_allow_truncation_is_true() {
  use crate::core::stage::Source;

  let framing = super::Framing::delimiter(vec![b'\n'], 1024, true);
  let source = Source::from(vec![b"hello\ntrailing".to_vec()]);
  let result = source.via(framing).collect_values();
  let frames = result.unwrap();
  assert_eq!(frames, vec![b"hello".to_vec(), b"trailing".to_vec()]);
}

#[test]
fn should_discard_trailing_bytes_when_allow_truncation_is_false() {
  use crate::core::stage::Source;

  let framing = super::Framing::delimiter(vec![b'\n'], 1024, false);
  let source = Source::from(vec![b"hello\ntrailing".to_vec()]);
  let result = source.via(framing).collect_values();
  let frames = result.unwrap();
  assert_eq!(frames, vec![b"hello".to_vec()]);
}

#[test]
fn should_error_when_frame_exceeds_max_frame_length() {
  use crate::core::{StreamError, stage::Source};

  let framing = super::Framing::delimiter(vec![b'\n'], 5, false);
  let source = Source::from(vec![b"toolong\nok".to_vec()]);
  let result = source.via(framing).collect_values();
  assert!(matches!(result, Err(StreamError::BufferOverflow)));
}

#[test]
fn should_error_when_buffer_exceeds_max_frame_length_without_delimiter() {
  use crate::core::{StreamError, stage::Source};

  let framing = super::Framing::delimiter(vec![b'\n'], 5, false);
  let source = Source::from(vec![b"abcdef".to_vec()]);
  let result = source.via(framing).collect_values();
  assert!(matches!(result, Err(StreamError::BufferOverflow)));
}

#[test]
fn should_create_length_field_flow() {
  use crate::core::stage::Source;

  let framing = super::Framing::length_field(0, 2);
  // frame1: length=3 (0x0003) + payload "abc"
  // frame2: length=2 (0x0002) + payload "de"
  let mut data = Vec::new();
  data.extend_from_slice(&[0x00, 0x03]);
  data.extend_from_slice(b"abc");
  data.extend_from_slice(&[0x00, 0x02]);
  data.extend_from_slice(b"de");

  let source = Source::single(data);
  let result = source.via(framing).collect_values();
  let frames = result.unwrap();
  assert_eq!(frames.len(), 2);
  assert_eq!(&frames[0][2..], b"abc");
  assert_eq!(&frames[1][2..], b"de");
}
