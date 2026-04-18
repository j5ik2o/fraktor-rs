use alloc::{boxed::Box, string::String, vec::Vec};

use crate::core::{
  StreamError,
  r#impl::CancellationCause,
  stage::{OutHandler, StageContext},
};

// --- テスト用ミニマルモック ---

/// on_pull だけ明示実装し、`on_downstream_finish` は trait デフォルト実装に委ねるモック。
///
/// デフォルト実装:
///   - on_downstream_finish → Ok(())（キャンセル伝播）
struct MinimalOutHandler {
  pulls: usize,
}

impl OutHandler<u32, u64> for MinimalOutHandler {
  fn on_pull(&mut self, _ctx: &mut dyn StageContext<u32, u64>) -> Result<(), StreamError> {
    self.pulls += 1;
    Ok(())
  }
}

/// デフォルト実装を上書きしてキャンセル原因を握るモック。
struct CapturingOutHandler {
  last_cause: Option<CancellationCause>,
}

impl OutHandler<u32, u64> for CapturingOutHandler {
  fn on_pull(&mut self, _ctx: &mut dyn StageContext<u32, u64>) -> Result<(), StreamError> {
    Ok(())
  }

  fn on_downstream_finish(
    &mut self,
    cause: CancellationCause,
    _ctx: &mut dyn StageContext<u32, u64>,
  ) -> Result<(), StreamError> {
    self.last_cause = Some(cause);
    Ok(())
  }
}

// --- トレイト trait 実装可能性 ---

#[test]
fn trait_can_be_implemented_with_only_on_pull() {
  fn assert_impls<T: OutHandler<u32, u64>>() {}
  assert_impls::<MinimalOutHandler>();
}

#[test]
fn trait_can_be_implemented_overriding_all_methods() {
  fn assert_impls<T: OutHandler<u32, u64>>() {}
  assert_impls::<CapturingOutHandler>();
}

#[test]
fn handler_struct_is_usable_as_trait_object() {
  let handler: Box<dyn OutHandler<u32, u64>> = Box::new(MinimalOutHandler { pulls: 0 });
  let _ = handler;
}

// --- ジェネリック可変性: 任意 <In, Out> で実装可能 ---

#[test]
fn trait_is_generic_over_in_out_types() {
  struct AnyTypes;

  impl<I, O> OutHandler<I, O> for AnyTypes {
    fn on_pull(&mut self, _ctx: &mut dyn StageContext<I, O>) -> Result<(), StreamError> {
      Ok(())
    }
  }

  fn assert_impls<T: OutHandler<String, Vec<u8>>>() {}
  assert_impls::<AnyTypes>();
}

#[test]
fn cancellation_cause_can_be_captured_via_override() {
  // on_downstream_finish override が CancellationCause を受け取れることを
  // コンパイル時型チェックで確認
  let mock = CapturingOutHandler { last_cause: None };
  // 初期状態は None
  assert!(mock.last_cause.is_none());
}
