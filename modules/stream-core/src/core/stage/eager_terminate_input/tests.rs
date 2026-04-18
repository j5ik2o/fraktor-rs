use crate::core::stage::{EagerTerminateInput, InHandler};

// --- コンストラクタ / Default ---

#[test]
fn default_constructs_unit_value() {
  // Given/When: Default で構築
  let handler = EagerTerminateInput::default();

  // Then: Debug フォーマットが空でないことで構築成功を確認
  let debug = alloc::format!("{:?}", handler);
  assert!(!debug.is_empty());
}

#[test]
fn new_constructs_unit_value() {
  // Given/When: 明示的な ::new() で構築（Default と等価であること）
  let via_new = EagerTerminateInput::new();
  let via_default = EagerTerminateInput::default();

  assert_eq!(via_new, via_default);
}

// --- Copy / Clone / 等価性 ---

#[test]
fn copy_semantics_work() {
  // Given: unit 相当の handler
  let a = EagerTerminateInput::default();
  let b = a;

  // Then: Copy により両方使用可能
  assert_eq!(a, b);
}

#[test]
fn clone_returns_equivalent_value() {
  let handler = EagerTerminateInput::default();
  let cloned = handler.clone();
  assert_eq!(handler, cloned);
}

#[test]
fn all_instances_are_equal() {
  // EagerTerminateInput は状態なし（unit struct 相当）のため、
  // いかなる 2 インスタンスも等価でなければならない
  assert_eq!(EagerTerminateInput::default(), EagerTerminateInput::default());
}

// --- Debug フォーマット ---

#[test]
fn debug_format_contains_type_name() {
  // Pekko の `override def toString = "EagerTerminateInput"` に対応
  let debug = alloc::format!("{:?}", EagerTerminateInput::default());
  assert!(debug.contains("EagerTerminateInput"), "Debug format was: {}", debug);
}

// --- InHandler trait 実装バウンド ---

#[test]
fn implements_in_handler_trait() {
  // InHandler<In, Out> trait 実装を具象ジェネリック引数で確認
  // ここで InHandler の型パラメータ数/位置が確定する（コンパイル時チェック）
  fn assert_impls<T: InHandler<u32, u64>>() {}
  assert_impls::<EagerTerminateInput>();
}
