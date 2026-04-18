use crate::core::StreamSubscriptionTimeoutTerminationMode;

// --- コンストラクティビリティ ---

#[test]
fn all_variants_are_constructible() {
  let _ = StreamSubscriptionTimeoutTerminationMode::Noop;
  let _ = StreamSubscriptionTimeoutTerminationMode::Warn;
  let _ = StreamSubscriptionTimeoutTerminationMode::Cancel;
}

// --- 等価性 ---

#[test]
fn same_variants_are_equal() {
  assert_eq!(StreamSubscriptionTimeoutTerminationMode::Noop, StreamSubscriptionTimeoutTerminationMode::Noop);
  assert_eq!(StreamSubscriptionTimeoutTerminationMode::Warn, StreamSubscriptionTimeoutTerminationMode::Warn);
  assert_eq!(StreamSubscriptionTimeoutTerminationMode::Cancel, StreamSubscriptionTimeoutTerminationMode::Cancel);
}

#[test]
fn different_variants_are_not_equal() {
  assert_ne!(StreamSubscriptionTimeoutTerminationMode::Noop, StreamSubscriptionTimeoutTerminationMode::Warn);
  assert_ne!(StreamSubscriptionTimeoutTerminationMode::Warn, StreamSubscriptionTimeoutTerminationMode::Cancel);
  assert_ne!(StreamSubscriptionTimeoutTerminationMode::Noop, StreamSubscriptionTimeoutTerminationMode::Cancel);
}

// --- Clone / Copy ---

#[test]
fn clone_preserves_variant() {
  let original = StreamSubscriptionTimeoutTerminationMode::Warn;
  let cloned = original;
  assert_eq!(original, cloned);
}

#[test]
fn copy_semantics_work() {
  let lhs = StreamSubscriptionTimeoutTerminationMode::Cancel;
  let rhs = lhs;
  assert_eq!(lhs, rhs);
}

// --- Debug フォーマット ---

#[test]
fn debug_format_contains_variant_name_noop() {
  let debug = alloc::format!("{:?}", StreamSubscriptionTimeoutTerminationMode::Noop);
  assert!(debug.contains("Noop"));
}

#[test]
fn debug_format_contains_variant_name_warn() {
  let debug = alloc::format!("{:?}", StreamSubscriptionTimeoutTerminationMode::Warn);
  assert!(debug.contains("Warn"));
}

#[test]
fn debug_format_contains_variant_name_cancel() {
  let debug = alloc::format!("{:?}", StreamSubscriptionTimeoutTerminationMode::Cancel);
  assert!(debug.contains("Cancel"));
}
