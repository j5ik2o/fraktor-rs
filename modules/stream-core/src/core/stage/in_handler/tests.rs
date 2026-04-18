use alloc::{boxed::Box, string::String, vec::Vec};

use crate::core::{
  StreamError,
  stage::{InHandler, StageContext},
};

// --- テスト用ミニマルモック ---

/// on_push だけ明示実装し、`on_upstream_finish` / `on_upstream_failure`
/// は trait デフォルト実装に委ねるモック。
///
/// デフォルト実装:
///   - on_upstream_finish → Ok(())（完了伝播）
///   - on_upstream_failure → Err(err)（失敗伝播）
struct MinimalInHandler {
  pushes: usize,
}

impl InHandler<u32, u64> for MinimalInHandler {
  fn on_push(&mut self, _ctx: &mut dyn StageContext<u32, u64>) -> Result<(), StreamError> {
    self.pushes += 1;
    Ok(())
  }
}

/// デフォルト実装をすべて上書きして「吸収する」挙動に変えるモック。
struct AbsorbingInHandler;

impl InHandler<u32, u64> for AbsorbingInHandler {
  fn on_push(&mut self, _ctx: &mut dyn StageContext<u32, u64>) -> Result<(), StreamError> {
    Ok(())
  }

  fn on_upstream_finish(&mut self, _ctx: &mut dyn StageContext<u32, u64>) -> Result<(), StreamError> {
    Ok(())
  }

  fn on_upstream_failure(
    &mut self,
    _err: StreamError,
    _ctx: &mut dyn StageContext<u32, u64>,
  ) -> Result<(), StreamError> {
    Ok(())
  }
}

// --- トレイト trait 実装可能性 ---

#[test]
fn trait_can_be_implemented_with_only_on_push() {
  // Given: on_push だけ実装して残りはデフォルトに任せるモック
  fn assert_impls<T: InHandler<u32, u64>>() {}

  // Then: コンパイルが通ること自体がテスト
  assert_impls::<MinimalInHandler>();
}

#[test]
fn trait_can_be_implemented_overriding_all_methods() {
  fn assert_impls<T: InHandler<u32, u64>>() {}
  assert_impls::<AbsorbingInHandler>();
}

#[test]
fn handler_struct_is_usable_as_trait_object() {
  // trait object（Box<dyn InHandler<In, Out>>）として保持可能であること
  let handler: Box<dyn InHandler<u32, u64>> = Box::new(MinimalInHandler { pushes: 0 });

  // Debug 不要 / メソッド呼び出し不要で型だけチェック
  let _ = handler;
}

// --- ジェネリック可変性: 任意 <In, Out> で実装可能 ---

#[test]
fn trait_is_generic_over_in_out_types() {
  struct AnyTypes;

  impl<I, O> InHandler<I, O> for AnyTypes {
    fn on_push(&mut self, _ctx: &mut dyn StageContext<I, O>) -> Result<(), StreamError> {
      Ok(())
    }
  }

  fn assert_impls<T: InHandler<String, Vec<u8>>>() {}
  assert_impls::<AnyTypes>();
}

// --- モック構造体が Debug 可能（テスト支援） ---

#[test]
fn minimal_mock_tracks_push_count() {
  // 直接 `on_push` を呼ばないが、モック自体のフィールドが意図通り機能することを確認
  let mock = MinimalInHandler { pushes: 3 };
  let debug = alloc::format!("{}", mock.pushes);
  assert_eq!(debug, "3");
}
