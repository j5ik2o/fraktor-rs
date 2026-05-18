use crate::r#impl::TimeoutKind;

// ---------------------------------------------------------------------------
// Variant construction
//   Pekko parity: BackpressureTimeoutException / CompletionTimeoutException /
//   StreamIdleTimeoutException / InitialTimeoutException (Pekko
//   `StreamTimeoutException.scala` sealed subtype hierarchy).
// ---------------------------------------------------------------------------

#[test]
fn backpressure_kind_is_constructible() {
  // Given/When: selecting the Backpressure variant
  let kind = TimeoutKind::Backpressure;

  // Then: it matches the expected variant
  assert!(matches!(kind, TimeoutKind::Backpressure));
}

#[test]
fn completion_kind_is_constructible() {
  // Given/When: selecting the Completion variant
  let kind = TimeoutKind::Completion;

  // Then: it matches the expected variant
  assert!(matches!(kind, TimeoutKind::Completion));
}

#[test]
fn idle_kind_is_constructible() {
  // Given/When: selecting the Idle variant
  let kind = TimeoutKind::Idle;

  // Then: it matches the expected variant
  assert!(matches!(kind, TimeoutKind::Idle));
}

#[test]
fn initial_kind_is_constructible() {
  // Given/When: selecting the Initial variant
  let kind = TimeoutKind::Initial;

  // Then: it matches the expected variant
  assert!(matches!(kind, TimeoutKind::Initial));
}

// ---------------------------------------------------------------------------
// Equality and distinctness
// ---------------------------------------------------------------------------

#[test]
fn variants_are_pairwise_distinct() {
  // Given: all four defined variants
  let kinds = [TimeoutKind::Backpressure, TimeoutKind::Completion, TimeoutKind::Idle, TimeoutKind::Initial];

  // Then: every pair of distinct variants is unequal (no variant collapses into another)
  for (i, left) in kinds.iter().enumerate() {
    for (j, right) in kinds.iter().enumerate() {
      if i != j {
        assert_ne!(left, right, "variants at indices {i} and {j} must be distinct");
      }
    }
  }
}

#[test]
fn same_variant_equals_itself() {
  // Given: the same variant in two bindings
  let a = TimeoutKind::Backpressure;
  let b = TimeoutKind::Backpressure;

  // Then: PartialEq yields equality
  assert_eq!(a, b);
}

// ---------------------------------------------------------------------------
// Copy / Clone semantics
//   Pekko 側の StreamTimeoutException は JVM 参照共有だが、fraktor-rs は値型として
//   複製可能な enum にする（変換コストゼロ・ロック不要）。
// ---------------------------------------------------------------------------

#[test]
fn kind_is_copy_via_let_binding() {
  // Given: a TimeoutKind value
  let original = TimeoutKind::Idle;

  // When: bind-copying (Copy semantics — compile-time proof also enforces the trait bound)
  let copied = original;

  // Then: the original is still usable AND both values are equal
  assert_eq!(original, copied);
}

#[test]
fn clone_returns_equal_value() {
  // Given: one variant of each
  for kind in [TimeoutKind::Backpressure, TimeoutKind::Completion, TimeoutKind::Idle, TimeoutKind::Initial] {
    // When: cloning
    let cloned = kind;

    // Then: the clone is equal to the original
    assert_eq!(kind, cloned);
  }
}

// ---------------------------------------------------------------------------
// Display rendering
//   Pekko parity: each subtype exposes `getMessage` derived from a human string,
//   but fraktor-rs keeps the label stable across reboots (lower-case identifier)
//   so that `StreamError::Timeout` can interpolate the kind deterministically.
//   The chosen identifiers match the existing `&'static str` payload already
//   emitted by backpressure_timeout_logic / completion_timeout_logic /
//   idle_timeout_logic / initial_timeout_logic.
// ---------------------------------------------------------------------------

#[test]
fn display_backpressure_matches_stable_label() {
  // Given: Backpressure
  let rendered = alloc::format!("{}", TimeoutKind::Backpressure);

  // Then: the label is the stable lower-case identifier
  assert_eq!(rendered, "backpressure");
}

#[test]
fn display_completion_matches_stable_label() {
  // Given: Completion
  let rendered = alloc::format!("{}", TimeoutKind::Completion);

  // Then: the label is the stable lower-case identifier
  assert_eq!(rendered, "completion");
}

#[test]
fn display_idle_matches_stable_label() {
  // Given: Idle
  let rendered = alloc::format!("{}", TimeoutKind::Idle);

  // Then: the label is the stable lower-case identifier
  assert_eq!(rendered, "idle");
}

#[test]
fn display_initial_matches_stable_label() {
  // Given: Initial
  let rendered = alloc::format!("{}", TimeoutKind::Initial);

  // Then: the label is the stable lower-case identifier
  assert_eq!(rendered, "initial");
}

#[test]
fn all_display_labels_are_distinct() {
  // Given: all four variants
  let labels = [
    alloc::format!("{}", TimeoutKind::Backpressure),
    alloc::format!("{}", TimeoutKind::Completion),
    alloc::format!("{}", TimeoutKind::Idle),
    alloc::format!("{}", TimeoutKind::Initial),
  ];

  // Then: no two variants produce the same Display string
  for (i, left) in labels.iter().enumerate() {
    for (j, right) in labels.iter().enumerate() {
      if i != j {
        assert_ne!(left, right, "labels at indices {i} and {j} must be distinct");
      }
    }
  }
}

// ---------------------------------------------------------------------------
// Debug rendering
//   Debug 出力は diagnostics 用途。Display と独立して variant 名が可視であること。
// ---------------------------------------------------------------------------

#[test]
fn debug_exposes_variant_name() {
  // Given: Backpressure
  let rendered = alloc::format!("{:?}", TimeoutKind::Backpressure);

  // Then: the Debug output contains the PascalCase variant identifier
  assert!(rendered.contains("Backpressure"), "Debug output must expose variant name: {rendered}");
}
