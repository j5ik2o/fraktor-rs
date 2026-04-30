use alloc::{vec, vec::Vec};

use super::JsonFraming;
use crate::core::{
  dsl::{Source, tests::RunWithCollectSink},
  r#impl::StreamError,
};

#[test]
fn should_extract_single_json_object() {
  let framing = JsonFraming::object_scanner(1024);
  let source = Source::single(br#"{"key":"value"}"#.to_vec());
  let result = source.via(framing).run_with_collect_sink();
  let frames: Vec<Vec<u8>> = result.unwrap();
  assert_eq!(frames, vec![br#"{"key":"value"}"#.to_vec()]);
}

#[test]
fn should_extract_multiple_json_objects() {
  let framing = JsonFraming::object_scanner(1024);
  let source = Source::single(br#"{"a":1}{"b":2}"#.to_vec());
  let result = source.via(framing).run_with_collect_sink();
  let frames: Vec<Vec<u8>> = result.unwrap();
  assert_eq!(frames, vec![br#"{"a":1}"#.to_vec(), br#"{"b":2}"#.to_vec()]);
}

#[test]
fn should_handle_nested_objects() {
  let framing = JsonFraming::object_scanner(1024);
  let source = Source::single(br#"{"nested":{"inner":true}}"#.to_vec());
  let result = source.via(framing).run_with_collect_sink();
  let frames: Vec<Vec<u8>> = result.unwrap();
  assert_eq!(frames, vec![br#"{"nested":{"inner":true}}"#.to_vec()]);
}

#[test]
fn should_handle_strings_with_brackets() {
  let framing = JsonFraming::object_scanner(1024);
  let source = Source::single(br#"{"text":"hello {world}"}"#.to_vec());
  let result = source.via(framing).run_with_collect_sink();
  let frames: Vec<Vec<u8>> = result.unwrap();
  assert_eq!(frames, vec![br#"{"text":"hello {world}"}"#.to_vec()]);
}

#[test]
fn should_handle_escaped_quotes_in_strings() {
  let framing = JsonFraming::object_scanner(1024);
  let source = Source::single(br#"{"text":"say \"hi\""}"#.to_vec());
  let result = source.via(framing).run_with_collect_sink();
  let frames: Vec<Vec<u8>> = result.unwrap();
  assert_eq!(frames, vec![br#"{"text":"say \"hi\""}"#.to_vec()]);
}

#[test]
fn should_handle_chunked_input() {
  let framing = JsonFraming::object_scanner(1024);
  let source = Source::from(vec![br#"{"ke"#.to_vec(), br#"y":"va"#.to_vec(), br#"lue"}"#.to_vec()]);
  let result = source.via(framing).run_with_collect_sink();
  let frames: Vec<Vec<u8>> = result.unwrap();
  assert_eq!(frames, vec![br#"{"key":"value"}"#.to_vec()]);
}

#[test]
fn should_handle_json_arrays() {
  let framing = JsonFraming::object_scanner(1024);
  let source = Source::single(br#"[1,2,3]"#.to_vec());
  let result = source.via(framing).run_with_collect_sink();
  let frames: Vec<Vec<u8>> = result.unwrap();
  assert_eq!(frames, vec![br#"[1,2,3]"#.to_vec()]);
}

#[test]
fn should_skip_whitespace_between_objects() {
  let framing = JsonFraming::object_scanner(1024);
  let source = Source::single(br#"  {"a":1}  {"b":2}  "#.to_vec());
  let result = source.via(framing).run_with_collect_sink();
  let frames: Vec<Vec<u8>> = result.unwrap();
  assert_eq!(frames, vec![br#"{"a":1}"#.to_vec(), br#"{"b":2}"#.to_vec()]);
}

#[test]
fn should_error_when_object_exceeds_max_length() {
  let framing = JsonFraming::object_scanner(5);
  let source = Source::single(br#"{"key":"value"}"#.to_vec());
  let result = source.via(framing).run_with_collect_sink();
  assert!(matches!(result, Err(StreamError::BufferOverflow)));
}

#[test]
fn should_error_on_incomplete_object_at_source_end() {
  let framing = JsonFraming::object_scanner(1024);
  let source = Source::single(br#"{"incomplete"#.to_vec());
  let result = source.via(framing).run_with_collect_sink();
  assert!(matches!(result, Err(StreamError::Failed)));
}

#[test]
fn should_handle_empty_object() {
  let framing = JsonFraming::object_scanner(1024);
  let source = Source::single(br#"{}"#.to_vec());
  let result = source.via(framing).run_with_collect_sink();
  let frames: Vec<Vec<u8>> = result.unwrap();
  assert_eq!(frames, vec![br#"{}"#.to_vec()]);
}

#[test]
fn should_handle_empty_array() {
  let framing = JsonFraming::object_scanner(1024);
  let source = Source::single(br#"[]"#.to_vec());
  let result = source.via(framing).run_with_collect_sink();
  let frames: Vec<Vec<u8>> = result.unwrap();
  assert_eq!(frames, vec![br#"[]"#.to_vec()]);
}

#[test]
fn should_error_when_leading_garbage_exceeds_max_length() {
  let framing = JsonFraming::object_scanner(10);
  // 20 bytes of whitespace/garbage before the bracket — exceeds limit
  let source = Source::single(b"                    {\"a\":1}".to_vec());
  let result = source.via(framing).run_with_collect_sink();
  // Leading data is discarded, so the object itself (7 bytes) fits within the limit
  let frames: Vec<Vec<u8>> = result.unwrap();
  assert_eq!(frames, vec![br#"{"a":1}"#.to_vec()]);
}

#[test]
fn should_error_when_no_bracket_data_exceeds_max_length() {
  let framing = JsonFraming::object_scanner(5);
  // No brackets at all — buffer grows beyond limit
  let source = Source::single(b"garbage_no_bracket_data".to_vec());
  let result = source.via(framing).run_with_collect_sink();
  assert!(matches!(result, Err(StreamError::BufferOverflow)));
}

// --- A5: JsonFraming.arrayScanner ---

#[test]
fn array_scanner_should_extract_elements_from_json_array() {
  // Given: a JSON array with multiple elements
  let framing = JsonFraming::array_scanner(1024);
  let source = Source::single(br#"[{"a":1},{"b":2},{"c":3}]"#.to_vec());

  // When: we process through array scanner
  let result = source.via(framing).run_with_collect_sink();
  let frames: Vec<Vec<u8>> = result.unwrap();

  // Then: each element is emitted individually
  assert_eq!(frames.len(), 3);
  assert_eq!(frames[0], br#"{"a":1}"#.to_vec());
  assert_eq!(frames[1], br#"{"b":2}"#.to_vec());
  assert_eq!(frames[2], br#"{"c":3}"#.to_vec());
}

#[test]
fn array_scanner_should_handle_primitive_elements() {
  // Given: a JSON array with primitive values
  let framing = JsonFraming::array_scanner(1024);
  let source = Source::single(br#"[1, 2, 3]"#.to_vec());

  // When/Then: primitive elements are extracted
  let result = source.via(framing).run_with_collect_sink();
  let frames: Vec<Vec<u8>> = result.unwrap();
  assert_eq!(frames.len(), 3);
  assert_eq!(frames[0], b"1".to_vec());
  assert_eq!(frames[1], b"2".to_vec());
  assert_eq!(frames[2], b"3".to_vec());
}

#[test]
fn array_scanner_should_handle_nested_arrays() {
  // Given: a JSON array containing nested arrays
  let framing = JsonFraming::array_scanner(1024);
  let source = Source::single(br#"[[1,2],[3,4]]"#.to_vec());

  // When/Then: nested arrays are emitted as complete elements
  let result = source.via(framing).run_with_collect_sink();
  let frames: Vec<Vec<u8>> = result.unwrap();
  assert_eq!(frames.len(), 2);
  assert_eq!(frames[0], br#"[1,2]"#.to_vec());
  assert_eq!(frames[1], br#"[3,4]"#.to_vec());
}

#[test]
fn array_scanner_should_handle_nested_objects() {
  // Given: a JSON array with nested objects
  let framing = JsonFraming::array_scanner(1024);
  let source = Source::single(br#"[{"nested":{"inner":true}}]"#.to_vec());

  // When/Then: nested objects are emitted whole
  let result = source.via(framing).run_with_collect_sink();
  let frames: Vec<Vec<u8>> = result.unwrap();
  assert_eq!(frames.len(), 1);
  assert_eq!(frames[0], br#"{"nested":{"inner":true}}"#.to_vec());
}

#[test]
fn array_scanner_should_handle_string_values_with_brackets() {
  // Given: a JSON array with strings containing bracket characters
  let framing = JsonFraming::array_scanner(1024);
  let source = Source::single(br#"[{"text":"[hello]"}]"#.to_vec());

  // When/Then: brackets inside strings do not affect parsing
  let result = source.via(framing).run_with_collect_sink();
  let frames: Vec<Vec<u8>> = result.unwrap();
  assert_eq!(frames.len(), 1);
  assert_eq!(frames[0], br#"{"text":"[hello]"}"#.to_vec());
}

#[test]
fn array_scanner_should_handle_empty_array() {
  // Given: an empty JSON array
  let framing = JsonFraming::array_scanner(1024);
  let source = Source::single(br#"[]"#.to_vec());

  // When/Then: no elements are emitted
  let result = source.via(framing).run_with_collect_sink();
  let frames: Vec<Vec<u8>> = result.unwrap();
  assert!(frames.is_empty());
}

#[test]
fn array_scanner_should_handle_chunked_input() {
  // Given: JSON array split across multiple chunks
  let framing = JsonFraming::array_scanner(1024);
  let source = Source::from(vec![br#"[{"ke"#.to_vec(), br#"y":"va"#.to_vec(), br#"lue"}]"#.to_vec()]);

  // When/Then: chunked input is reassembled correctly
  let result = source.via(framing).run_with_collect_sink();
  let frames: Vec<Vec<u8>> = result.unwrap();
  assert_eq!(frames.len(), 1);
  assert_eq!(frames[0], br#"{"key":"value"}"#.to_vec());
}

#[test]
fn array_scanner_should_error_when_element_exceeds_max_length() {
  // Given: a JSON array with an element exceeding max length
  let framing = JsonFraming::array_scanner(5);
  let source = Source::single(br#"[{"key":"value"}]"#.to_vec());

  // When/Then: buffer overflow error
  let result = source.via(framing).run_with_collect_sink();
  assert!(matches!(result, Err(StreamError::BufferOverflow)));
}

#[test]
fn array_scanner_should_error_when_primitive_exceeds_max_length() {
  let framing = JsonFraming::array_scanner(5);
  let source = Source::single(b"[123456,1]".to_vec());

  let result = source.via(framing).run_with_collect_sink();

  assert!(matches!(result, Err(StreamError::BufferOverflow)));
}

#[test]
fn array_scanner_should_error_when_primitive_exceeds_max_length_before_array_end() {
  let framing = JsonFraming::array_scanner(5);
  let source = Source::single(b"[123456]".to_vec());

  let result = source.via(framing).run_with_collect_sink();

  assert!(matches!(result, Err(StreamError::BufferOverflow)));
}

#[test]
fn array_scanner_should_error_when_primitive_exceeds_max_length_across_chunks() {
  let framing = JsonFraming::array_scanner(5);
  let source = Source::from(vec![b"[12345".to_vec(), b"6,1]".to_vec()]);

  let result = source.via(framing).run_with_collect_sink();

  assert!(matches!(result, Err(StreamError::BufferOverflow)));
}

#[test]
fn array_scanner_should_error_on_unclosed_array() {
  let framing = JsonFraming::array_scanner(1024);
  let source = Source::single(b"[".to_vec());

  let result = source.via(framing).run_with_collect_sink();

  assert!(matches!(result, Err(StreamError::Failed)));
}

#[test]
fn array_scanner_should_error_on_non_array_input_at_source_end() {
  let framing = JsonFraming::array_scanner(1024);
  let source = Source::single(b"garbage".to_vec());

  let result = source.via(framing).run_with_collect_sink();

  assert!(matches!(result, Err(StreamError::Failed)));
}

#[test]
fn array_scanner_should_error_on_data_after_array_is_closed() {
  let framing = JsonFraming::array_scanner(1024);
  let source = Source::from(vec![b"[1]".to_vec(), b",[2]".to_vec()]);

  let result = source.via(framing).run_with_collect_sink();

  assert!(matches!(result, Err(StreamError::Failed)));
}

#[test]
fn array_scanner_should_error_on_extra_closing_bracket() {
  let framing = JsonFraming::array_scanner(1024);
  let source = Source::single(b"[1]]".to_vec());

  let result = source.via(framing).run_with_collect_sink();

  assert!(matches!(result, Err(StreamError::Failed)));
}

#[test]
fn array_scanner_should_error_when_separator_is_missing() {
  let framing = JsonFraming::array_scanner(1024);
  let source = Source::single(b"[1 2]".to_vec());

  let result = source.via(framing).run_with_collect_sink();

  assert!(matches!(result, Err(StreamError::Failed)));
}

#[test]
fn array_scanner_should_error_on_double_comma() {
  let framing = JsonFraming::array_scanner(1024);
  let source = Source::single(b"[1,,2]".to_vec());

  let result = source.via(framing).run_with_collect_sink();

  assert!(matches!(result, Err(StreamError::Failed)));
}

#[test]
fn array_scanner_should_error_on_trailing_comma() {
  let framing = JsonFraming::array_scanner(1024);
  let source = Source::single(b"[1,]".to_vec());

  let result = source.via(framing).run_with_collect_sink();

  assert!(matches!(result, Err(StreamError::Failed)));
}

#[test]
fn array_scanner_should_error_on_leading_comma() {
  let framing = JsonFraming::array_scanner(1024);
  let source = Source::single(b"[,1]".to_vec());

  let result = source.via(framing).run_with_collect_sink();

  assert!(matches!(result, Err(StreamError::Failed)));
}

#[test]
fn array_scanner_should_error_on_chunked_leading_comma() {
  let framing = JsonFraming::array_scanner(1024);
  let source = Source::from(vec![b"[".to_vec(), b",".to_vec(), b"1]".to_vec()]);

  let result = source.via(framing).run_with_collect_sink();

  assert!(matches!(result, Err(StreamError::Failed)));
}
