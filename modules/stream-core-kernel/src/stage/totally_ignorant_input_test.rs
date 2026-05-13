use crate::stage::{InHandler, TotallyIgnorantInput};

// --- コンストラクタ / Default ---

#[test]
fn default_constructs_unit_value() {
  let handler = TotallyIgnorantInput::default();
  let debug = alloc::format!("{:?}", handler);
  assert!(!debug.is_empty());
}

#[test]
fn new_constructs_unit_value() {
  let via_new = TotallyIgnorantInput::new();
  let via_default = TotallyIgnorantInput::default();
  assert_eq!(via_new, via_default);
}

// --- Copy / Clone / 等価性 ---

#[test]
fn copy_semantics_work() {
  let a = TotallyIgnorantInput::default();
  let b = a;
  assert_eq!(a, b);
}

#[test]
fn clone_returns_equivalent_value() {
  let handler = TotallyIgnorantInput::default();
  let cloned = handler.clone();
  assert_eq!(handler, cloned);
}

#[test]
fn all_instances_are_equal() {
  assert_eq!(TotallyIgnorantInput::default(), TotallyIgnorantInput::default());
}

// --- Debug フォーマット ---

#[test]
fn debug_format_contains_type_name() {
  // Pekko 側は `toString` を override していないが、Rust 側の可読性のため
  // Debug 出力に型名が含まれることを確認（derive(Debug) の標準挙動）
  let debug = alloc::format!("{:?}", TotallyIgnorantInput::default());
  assert!(debug.contains("TotallyIgnorantInput"), "Debug format was: {}", debug);
}

// --- InHandler trait 実装バウンド ---

#[test]
fn implements_in_handler_trait() {
  // TotallyIgnorantInput は `on_push` / `on_upstream_finish` / `on_upstream_failure`
  // のすべてを override して Ok(()) を返す（= 失敗も含めて全イベント吸収）。
  // rustdoc に明記された「呼び出し側の責任でユースケース限定」前提。
  fn assert_impls<T: InHandler<u32, u64>>() {}
  assert_impls::<TotallyIgnorantInput>();
}
