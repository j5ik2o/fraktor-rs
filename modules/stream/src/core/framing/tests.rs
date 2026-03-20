use alloc::{boxed::Box, vec, vec::Vec};

use super::{
  SimpleFramingDecoderLogic, checked_frame_length, find_delimiter, read_big_endian_i32, read_big_endian_uint,
};

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
fn should_read_big_endian_i32() {
  assert_eq!(read_big_endian_i32([0x00, 0x00, 0x01, 0x00]), 256);
}

#[test]
fn should_error_when_frame_length_addition_overflows() {
  use crate::core::StreamError;

  let result = checked_frame_length(usize::MAX, 1);
  assert!(matches!(result, Err(StreamError::BufferOverflow)));
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

#[test]
fn should_encode_payload_with_four_byte_big_endian_length_header() {
  use crate::core::stage::Source;

  let protocol = super::Framing::simple_framing_protocol(16);
  let (encoder, _decoder, _mat) = protocol.split();

  let frames = Source::single(b"abc".to_vec()).via(encoder).collect_values().unwrap();
  assert_eq!(frames, vec![vec![0x00, 0x00, 0x00, 0x03, b'a', b'b', b'c']]);
}

#[test]
fn should_decode_chunked_frames_and_strip_length_header() {
  use crate::core::stage::Source;

  let protocol = super::Framing::simple_framing_protocol(16);
  let (_encoder, decoder, _mat) = protocol.split();

  let frames = Source::from(vec![vec![0x00, 0x00, 0x00], vec![0x03, b'a'], vec![b'b', b'c', 0x00, 0x00], vec![
    0x00, 0x02, b'd', b'e',
  ]])
  .via(decoder)
  .collect_values()
  .unwrap();

  assert_eq!(frames, vec![b"abc".to_vec(), b"de".to_vec()]);
}

#[test]
fn should_support_empty_payload_frame() {
  use crate::core::stage::Source;

  let protocol = super::Framing::simple_framing_protocol(16);
  let (encoder, decoder, _mat) = protocol.split();

  let encoded = Source::single(Vec::new()).via(encoder).collect_values().unwrap();
  assert_eq!(encoded, vec![vec![0x00, 0x00, 0x00, 0x00]]);

  let decoded = Source::from(encoded).via(decoder).collect_values().unwrap();
  assert_eq!(decoded, vec![Vec::new()]);
}

#[test]
fn should_round_trip_payloads_through_simple_framing_protocol() {
  use crate::core::stage::{Source, flow::Flow};

  let protocol = super::Framing::simple_framing_protocol(16);
  let loopback = protocol.join(Flow::new());

  let decoded = Source::from(vec![b"abc".to_vec(), Vec::new(), b"de".to_vec()]).via(loopback).collect_values().unwrap();

  assert_eq!(decoded, vec![b"abc".to_vec(), Vec::new(), b"de".to_vec()]);
}

#[test]
fn should_error_when_payload_exceeds_maximum_message_length() {
  use crate::core::{StreamError, stage::Source};

  let protocol = super::Framing::simple_framing_protocol(3);
  let (encoder, _decoder, _mat) = protocol.split();

  let result = Source::single(b"toolong".to_vec()).via(encoder).collect_values();
  assert!(matches!(result, Err(StreamError::BufferOverflow)));
}

#[test]
fn should_error_when_decoded_length_header_exceeds_maximum_message_length() {
  use crate::core::{StreamError, stage::Source};

  let protocol = super::Framing::simple_framing_protocol(3);
  let (_encoder, decoder, _mat) = protocol.split();

  let result = Source::single(vec![0x00, 0x00, 0x00, 0x04, b'a', b'b', b'c', b'd']).via(decoder).collect_values();

  assert!(matches!(result, Err(StreamError::BufferOverflow)));
}

#[test]
fn should_error_when_source_ends_with_truncated_decoded_frame() {
  use crate::core::{StreamError, stage::Source};

  let protocol = super::Framing::simple_framing_protocol(16);
  let (_encoder, decoder, _mat) = protocol.split();

  let result = Source::single(vec![0x00, 0x00, 0x00, 0x03, b'a', b'b']).via(decoder).collect_values();

  assert!(matches!(result, Err(StreamError::Failed)));
}

#[test]
fn should_fail_decoder_immediately_when_length_header_is_negative() {
  use crate::core::{FlowLogic, StreamError};

  let mut logic = SimpleFramingDecoderLogic { maximum_message_length: usize::MAX, buffer: Vec::new() };
  let result = logic.apply(Box::new(vec![0x80, 0x00, 0x00, 0x00, b'a']));

  assert!(matches!(result, Err(StreamError::Failed)));
}
