use crate::stage::{IgnoreTerminateInput, InHandler};

// --- コンストラクタ / Default ---

#[test]
fn default_constructs_unit_value() {
  let handler = IgnoreTerminateInput::default();
  let debug = alloc::format!("{:?}", handler);
  assert!(!debug.is_empty());
}

#[test]
fn new_constructs_unit_value() {
  let via_new = IgnoreTerminateInput::new();
  let via_default = IgnoreTerminateInput::default();
  assert_eq!(via_new, via_default);
}

// --- Copy / Clone / 等価性 ---

#[test]
fn copy_semantics_work() {
  let a = IgnoreTerminateInput::default();
  let b = a;
  assert_eq!(a, b);
}

#[test]
fn clone_returns_equivalent_value() {
  let handler = IgnoreTerminateInput::default();
  let cloned = handler.clone();
  assert_eq!(handler, cloned);
}

#[test]
fn all_instances_are_equal() {
  assert_eq!(IgnoreTerminateInput::default(), IgnoreTerminateInput::default());
}

// --- Debug フォーマット ---

#[test]
fn debug_format_contains_type_name() {
  // Pekko の `override def toString = "IgnoreTerminateInput"` に対応
  let debug = alloc::format!("{:?}", IgnoreTerminateInput::default());
  assert!(debug.contains("IgnoreTerminateInput"), "Debug format was: {}", debug);
}

// --- InHandler trait 実装バウンド ---

#[test]
fn implements_in_handler_trait() {
  // IgnoreTerminateInput は `on_upstream_finish` を override して Ok(()) を返す（= 吸収）
  // `on_upstream_failure` はデフォルト実装（= 失敗伝播）
  fn assert_impls<T: InHandler<u32, u64>>() {}
  assert_impls::<IgnoreTerminateInput>();
}
