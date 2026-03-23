use crate::core::CancellationStrategyKind;

// --- variant construction ---

#[test]
fn all_variants_are_constructible() {
  let _ = CancellationStrategyKind::CompleteStage;
  let _ = CancellationStrategyKind::FailStage;
  let _ = CancellationStrategyKind::PropagateFailure;
}

// --- equality ---

#[test]
fn same_variants_are_equal() {
  assert_eq!(CancellationStrategyKind::CompleteStage, CancellationStrategyKind::CompleteStage);
  assert_eq!(CancellationStrategyKind::FailStage, CancellationStrategyKind::FailStage);
  assert_eq!(CancellationStrategyKind::PropagateFailure, CancellationStrategyKind::PropagateFailure);
}

#[test]
fn different_variants_are_not_equal() {
  assert_ne!(CancellationStrategyKind::CompleteStage, CancellationStrategyKind::FailStage);
  assert_ne!(CancellationStrategyKind::FailStage, CancellationStrategyKind::PropagateFailure);
  assert_ne!(CancellationStrategyKind::CompleteStage, CancellationStrategyKind::PropagateFailure);
}

// --- clone ---

#[test]
fn clone_preserves_variant() {
  let original = CancellationStrategyKind::FailStage;
  let cloned = original.clone();
  assert_eq!(original, cloned);
}

// --- copy ---

#[test]
fn copy_semantics_work() {
  let a = CancellationStrategyKind::PropagateFailure;
  let b = a;
  assert_eq!(a, b);
}

// --- debug ---

#[test]
fn debug_format_is_non_empty() {
  let debug = alloc::format!("{:?}", CancellationStrategyKind::CompleteStage);
  assert!(!debug.is_empty());
}
