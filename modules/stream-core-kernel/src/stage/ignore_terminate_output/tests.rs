use crate::stage::{IgnoreTerminateOutput, OutHandler};

// --- コンストラクタ / Default ---

#[test]
fn default_constructs_unit_value() {
  let handler = IgnoreTerminateOutput::default();
  let debug = alloc::format!("{:?}", handler);
  assert!(!debug.is_empty());
}

#[test]
fn new_constructs_unit_value() {
  let via_new = IgnoreTerminateOutput::new();
  let via_default = IgnoreTerminateOutput::default();
  assert_eq!(via_new, via_default);
}

// --- Copy / Clone / 等価性 ---

#[test]
fn copy_semantics_work() {
  let a = IgnoreTerminateOutput::default();
  let b = a;
  assert_eq!(a, b);
}

#[test]
fn clone_returns_equivalent_value() {
  let handler = IgnoreTerminateOutput::default();
  let cloned = handler.clone();
  assert_eq!(handler, cloned);
}

#[test]
fn all_instances_are_equal() {
  assert_eq!(IgnoreTerminateOutput::default(), IgnoreTerminateOutput::default());
}

// --- Debug フォーマット ---

#[test]
fn debug_format_contains_type_name() {
  // Pekko の `override def toString = "IgnoreTerminateOutput"` に対応
  let debug = alloc::format!("{:?}", IgnoreTerminateOutput::default());
  assert!(debug.contains("IgnoreTerminateOutput"), "Debug format was: {}", debug);
}

// --- OutHandler trait 実装バウンド ---

#[test]
fn implements_out_handler_trait() {
  // IgnoreTerminateOutput は `on_downstream_finish` を override して Ok(()) を返す（= 吸収）
  fn assert_impls<T: OutHandler<u32, u64>>() {}
  assert_impls::<IgnoreTerminateOutput>();
}
