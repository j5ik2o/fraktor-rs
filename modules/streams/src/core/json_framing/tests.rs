use alloc::{vec, vec::Vec};

use crate::core::{StreamError, json_framing::JsonFraming, stage::Source};

#[test]
fn should_extract_single_json_object() {
  let framing = JsonFraming::object_scanner(1024);
  let source = Source::single(br#"{"key":"value"}"#.to_vec());
  let result = source.via(framing).collect_values();
  let frames: Vec<Vec<u8>> = result.unwrap();
  assert_eq!(frames, vec![br#"{"key":"value"}"#.to_vec()]);
}

#[test]
fn should_extract_multiple_json_objects() {
  let framing = JsonFraming::object_scanner(1024);
  let source = Source::single(br#"{"a":1}{"b":2}"#.to_vec());
  let result = source.via(framing).collect_values();
  let frames: Vec<Vec<u8>> = result.unwrap();
  assert_eq!(frames, vec![br#"{"a":1}"#.to_vec(), br#"{"b":2}"#.to_vec()]);
}

#[test]
fn should_handle_nested_objects() {
  let framing = JsonFraming::object_scanner(1024);
  let source = Source::single(br#"{"nested":{"inner":true}}"#.to_vec());
  let result = source.via(framing).collect_values();
  let frames: Vec<Vec<u8>> = result.unwrap();
  assert_eq!(frames, vec![br#"{"nested":{"inner":true}}"#.to_vec()]);
}

#[test]
fn should_handle_strings_with_brackets() {
  let framing = JsonFraming::object_scanner(1024);
  let source = Source::single(br#"{"text":"hello {world}"}"#.to_vec());
  let result = source.via(framing).collect_values();
  let frames: Vec<Vec<u8>> = result.unwrap();
  assert_eq!(frames, vec![br#"{"text":"hello {world}"}"#.to_vec()]);
}

#[test]
fn should_handle_escaped_quotes_in_strings() {
  let framing = JsonFraming::object_scanner(1024);
  let source = Source::single(br#"{"text":"say \"hi\""}"#.to_vec());
  let result = source.via(framing).collect_values();
  let frames: Vec<Vec<u8>> = result.unwrap();
  assert_eq!(frames, vec![br#"{"text":"say \"hi\""}"#.to_vec()]);
}

#[test]
fn should_handle_chunked_input() {
  let framing = JsonFraming::object_scanner(1024);
  let source = Source::from(vec![br#"{"ke"#.to_vec(), br#"y":"va"#.to_vec(), br#"lue"}"#.to_vec()]);
  let result = source.via(framing).collect_values();
  let frames: Vec<Vec<u8>> = result.unwrap();
  assert_eq!(frames, vec![br#"{"key":"value"}"#.to_vec()]);
}

#[test]
fn should_handle_json_arrays() {
  let framing = JsonFraming::object_scanner(1024);
  let source = Source::single(br#"[1,2,3]"#.to_vec());
  let result = source.via(framing).collect_values();
  let frames: Vec<Vec<u8>> = result.unwrap();
  assert_eq!(frames, vec![br#"[1,2,3]"#.to_vec()]);
}

#[test]
fn should_skip_whitespace_between_objects() {
  let framing = JsonFraming::object_scanner(1024);
  let source = Source::single(br#"  {"a":1}  {"b":2}  "#.to_vec());
  let result = source.via(framing).collect_values();
  let frames: Vec<Vec<u8>> = result.unwrap();
  assert_eq!(frames, vec![br#"{"a":1}"#.to_vec(), br#"{"b":2}"#.to_vec()]);
}

#[test]
fn should_error_when_object_exceeds_max_length() {
  let framing = JsonFraming::object_scanner(5);
  let source = Source::single(br#"{"key":"value"}"#.to_vec());
  let result = source.via(framing).collect_values();
  assert!(matches!(result, Err(StreamError::BufferOverflow)));
}

#[test]
fn should_error_on_incomplete_object_at_source_end() {
  let framing = JsonFraming::object_scanner(1024);
  let source = Source::single(br#"{"incomplete"#.to_vec());
  let result = source.via(framing).collect_values();
  assert!(matches!(result, Err(StreamError::Failed)));
}

#[test]
fn should_handle_empty_object() {
  let framing = JsonFraming::object_scanner(1024);
  let source = Source::single(br#"{}"#.to_vec());
  let result = source.via(framing).collect_values();
  let frames: Vec<Vec<u8>> = result.unwrap();
  assert_eq!(frames, vec![br#"{}"#.to_vec()]);
}

#[test]
fn should_handle_empty_array() {
  let framing = JsonFraming::object_scanner(1024);
  let source = Source::single(br#"[]"#.to_vec());
  let result = source.via(framing).collect_values();
  let frames: Vec<Vec<u8>> = result.unwrap();
  assert_eq!(frames, vec![br#"[]"#.to_vec()]);
}

#[test]
fn should_error_when_leading_garbage_exceeds_max_length() {
  let framing = JsonFraming::object_scanner(10);
  // 20 bytes of whitespace/garbage before the bracket — exceeds limit
  let source = Source::single(b"                    {\"a\":1}".to_vec());
  let result = source.via(framing).collect_values();
  // Leading data is discarded, so the object itself (7 bytes) fits within the limit
  let frames: Vec<Vec<u8>> = result.unwrap();
  assert_eq!(frames, vec![br#"{"a":1}"#.to_vec()]);
}

#[test]
fn should_error_when_no_bracket_data_exceeds_max_length() {
  let framing = JsonFraming::object_scanner(5);
  // No brackets at all — buffer grows beyond limit
  let source = Source::single(b"garbage_no_bracket_data".to_vec());
  let result = source.via(framing).collect_values();
  assert!(matches!(result, Err(StreamError::BufferOverflow)));
}
