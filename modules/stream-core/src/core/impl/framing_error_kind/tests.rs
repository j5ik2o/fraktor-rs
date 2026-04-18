use alloc::string::String;

use crate::core::r#impl::FramingErrorKind;

// ---------------------------------------------------------------------------
// FrameTooLarge variant
//   Pekko parity: `FramingException("Maximum allowed message size is $max but
//   tried to send $actual bytes")` (Framing.scala:196). fraktor-rs splits the
//   size pair (actual, max) into explicit fields so that the caller can render
//   a localized or restructured message without re-parsing the text.
// ---------------------------------------------------------------------------

#[test]
fn frame_too_large_is_constructible_with_actual_and_max() {
  // Given/When: constructing FrameTooLarge with distinct sizes
  let kind = FramingErrorKind::FrameTooLarge { actual: 8192, max: 4096 };

  // Then: the variant matches and exposes both values
  assert!(matches!(kind, FramingErrorKind::FrameTooLarge { actual: 8192, max: 4096 }));
}

#[test]
fn frame_too_large_display_contains_both_sizes() {
  // Given: a FrameTooLarge with actual=8192, max=4096
  let kind = FramingErrorKind::FrameTooLarge { actual: 8192, max: 4096 };

  // When: formatting with Display
  let rendered = alloc::format!("{kind}");

  // Then: both the actual and the max sizes are present in the rendering
  //   (Pekko exposes them as a single sentence — the fraktor-rs rendering must
  //    also show both numbers to preserve diagnostic parity).
  assert!(rendered.contains("8192"), "rendered output must contain actual size: {rendered}");
  assert!(rendered.contains("4096"), "rendered output must contain max size: {rendered}");
}

#[test]
fn frame_too_large_preserves_value_on_clone() {
  // Given: a FrameTooLarge
  let original = FramingErrorKind::FrameTooLarge { actual: 10, max: 5 };

  // When: cloning
  let cloned = original.clone();

  // Then: equality is preserved
  assert_eq!(original, cloned);
}

#[test]
fn frame_too_large_distinct_by_actual_are_unequal() {
  // Given: two FrameTooLarge with the same max but distinct actual
  let first = FramingErrorKind::FrameTooLarge { actual: 10, max: 5 };
  let second = FramingErrorKind::FrameTooLarge { actual: 11, max: 5 };

  // Then: they are not equal
  assert_ne!(first, second);
}

#[test]
fn frame_too_large_distinct_by_max_are_unequal() {
  // Given: two FrameTooLarge with the same actual but distinct max
  let first = FramingErrorKind::FrameTooLarge { actual: 10, max: 5 };
  let second = FramingErrorKind::FrameTooLarge { actual: 10, max: 6 };

  // Then: they are not equal
  assert_ne!(first, second);
}

#[test]
fn frame_too_large_accepts_boundary_value_zero() {
  // Given: a FrameTooLarge at the numeric boundary (actual > max with max == 0)
  //   Boundary test: ensures the variant does not reject zero-sized max, which
  //   is a legitimate configuration (reject-all framing).
  let kind = FramingErrorKind::FrameTooLarge { actual: 1, max: 0 };

  // When/Then: the variant round-trips through clone
  assert_eq!(kind.clone(), kind);
}

// ---------------------------------------------------------------------------
// Malformed variant
//   Pekko parity: `FramingException(msg)` with free-form diagnostic messages
//   such as "Stream finished but there was a truncated final frame in the buffer"
//   (Framing.scala:256). fraktor-rs wraps the original message string so that
//   upstream diagnostics are preserved verbatim.
// ---------------------------------------------------------------------------

#[test]
fn malformed_is_constructible_with_message() {
  // Given/When: constructing Malformed with a diagnostic message
  let kind = FramingErrorKind::Malformed(String::from("unexpected delimiter"));

  // Then: the variant matches
  assert!(matches!(kind, FramingErrorKind::Malformed(_)));
}

#[test]
fn malformed_display_contains_message_verbatim() {
  // Given: a Malformed with a Pekko-style diagnostic message
  //   Pekko reference: "Stream finished but there was a truncated final frame
  //                     in the buffer" (Framing.scala:256).
  let msg = "Stream finished but there was a truncated final frame in the buffer";
  let kind = FramingErrorKind::Malformed(String::from(msg));

  // When: formatting with Display
  let rendered = alloc::format!("{kind}");

  // Then: the full diagnostic message appears verbatim in the rendering
  assert!(rendered.contains(msg), "rendered output must contain the original message verbatim: {rendered}");
}

#[test]
fn malformed_preserves_value_on_clone() {
  // Given: a Malformed error
  let original = FramingErrorKind::Malformed(String::from("truncated frame"));

  // When: cloning
  let cloned = original.clone();

  // Then: equality is preserved
  assert_eq!(original, cloned);
}

#[test]
fn malformed_with_distinct_messages_are_unequal() {
  // Given: two Malformed with distinct payloads
  let first = FramingErrorKind::Malformed(String::from("a"));
  let second = FramingErrorKind::Malformed(String::from("b"));

  // Then: they are not equal
  assert_ne!(first, second);
}

#[test]
fn malformed_accepts_empty_message() {
  // Given: a Malformed with an empty payload (no diagnostic available)
  //   Boundary test: empty String must remain round-trippable.
  let kind = FramingErrorKind::Malformed(String::new());

  // When/Then: cloning preserves equality
  assert_eq!(kind.clone(), kind);
}

// ---------------------------------------------------------------------------
// Cross-variant distinctness
// ---------------------------------------------------------------------------

#[test]
fn frame_too_large_is_distinct_from_malformed() {
  // Given: one variant of each kind
  let too_large = FramingErrorKind::FrameTooLarge { actual: 1, max: 0 };
  let malformed = FramingErrorKind::Malformed(String::from("x"));

  // Then: they are not equal at the enum level
  assert_ne!(too_large, malformed);
}

// ---------------------------------------------------------------------------
// Debug rendering
// ---------------------------------------------------------------------------

#[test]
fn debug_exposes_variant_name_for_frame_too_large() {
  // Given: a FrameTooLarge error
  let kind = FramingErrorKind::FrameTooLarge { actual: 10, max: 5 };

  // When: formatting with Debug
  let rendered = alloc::format!("{kind:?}");

  // Then: the Debug output names the variant explicitly
  assert!(rendered.contains("FrameTooLarge"), "Debug output must expose variant name: {rendered}");
}

#[test]
fn debug_exposes_variant_name_for_malformed() {
  // Given: a Malformed error
  let kind = FramingErrorKind::Malformed(String::from("xyz"));

  // When: formatting with Debug
  let rendered = alloc::format!("{kind:?}");

  // Then: the Debug output names the variant explicitly
  assert!(rendered.contains("Malformed"), "Debug output must expose variant name: {rendered}");
}
