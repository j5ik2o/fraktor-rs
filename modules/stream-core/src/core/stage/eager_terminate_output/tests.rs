use crate::core::stage::{EagerTerminateOutput, OutHandler};

// --- コンストラクタ / Default ---

#[test]
fn default_constructs_unit_value() {
  let handler = EagerTerminateOutput::default();
  let debug = alloc::format!("{:?}", handler);
  assert!(!debug.is_empty());
}

#[test]
fn new_constructs_unit_value() {
  let via_new = EagerTerminateOutput::new();
  let via_default = EagerTerminateOutput::default();
  assert_eq!(via_new, via_default);
}

// --- Copy / Clone / 等価性 ---

#[test]
fn copy_semantics_work() {
  let a = EagerTerminateOutput::default();
  let b = a;
  assert_eq!(a, b);
}

#[test]
fn clone_returns_equivalent_value() {
  let handler = EagerTerminateOutput::default();
  let cloned = handler.clone();
  assert_eq!(handler, cloned);
}

#[test]
fn all_instances_are_equal() {
  // unit struct 相当のため 2 インスタンスは常に等価
  assert_eq!(EagerTerminateOutput::default(), EagerTerminateOutput::default());
}

// --- Debug フォーマット ---

#[test]
fn debug_format_contains_type_name() {
  // Pekko の `override def toString = "EagerTerminateOutput"` に対応
  let debug = alloc::format!("{:?}", EagerTerminateOutput::default());
  assert!(debug.contains("EagerTerminateOutput"), "Debug format was: {}", debug);
}

// --- OutHandler trait 実装バウンド ---

#[test]
fn implements_out_handler_trait() {
  fn assert_impls<T: OutHandler<u32, u64>>() {}
  assert_impls::<EagerTerminateOutput>();
}
