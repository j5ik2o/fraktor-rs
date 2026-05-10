use alloc::boxed::Box;

use super::SubSinkInletHandler;
use crate::StreamError;

struct MinimalSubSinkInletHandler {
  pushes: usize,
}

impl SubSinkInletHandler<u32> for MinimalSubSinkInletHandler {
  fn on_push(&mut self) -> Result<(), StreamError> {
    self.pushes = self.pushes.saturating_add(1);
    Ok(())
  }
}

struct AbsorbingSubSinkInletHandler;

impl SubSinkInletHandler<u32> for AbsorbingSubSinkInletHandler {
  fn on_push(&mut self) -> Result<(), StreamError> {
    Ok(())
  }

  fn on_upstream_finish(&mut self) -> Result<(), StreamError> {
    Ok(())
  }

  fn on_upstream_failure(&mut self, _error: StreamError) -> Result<(), StreamError> {
    Ok(())
  }
}

#[test]
fn trait_can_be_implemented_with_only_on_push() {
  // Given: on_push だけを明示実装する handler
  let mut handler = MinimalSubSinkInletHandler { pushes: 0 };

  // When: push callback を呼び出す
  let result = handler.on_push();

  // Then: default termination callback を追加実装しなくても成立する
  assert_eq!(result, Ok(()));
  assert_eq!(handler.pushes, 1);
  assert_eq!(handler.on_upstream_finish(), Ok(()));
}

#[test]
fn default_upstream_failure_re_raises_error() {
  // Given: upstream failure を上書きしない handler
  let mut handler = MinimalSubSinkInletHandler { pushes: 0 };

  // When: upstream failure callback を呼び出す
  let result = handler.on_upstream_failure(StreamError::Failed);

  // Then: Pekko の InHandler と同じく failure は伝播される
  assert_eq!(result, Err(StreamError::Failed));
}

#[test]
fn trait_can_absorb_all_callbacks_when_overridden() {
  // Given: すべての callback を吸収する handler
  let mut handler = AbsorbingSubSinkInletHandler;

  // When: termination callback を呼び出す
  let failure = handler.on_upstream_failure(StreamError::Failed);

  // Then: override により failure を吸収できる
  assert_eq!(handler.on_push(), Ok(()));
  assert_eq!(handler.on_upstream_finish(), Ok(()));
  assert_eq!(failure, Ok(()));
}

#[test]
fn handler_is_usable_as_trait_object() {
  // Given: 動的 substream port が handler を trait object として保持できる
  let handler: Box<dyn SubSinkInletHandler<u32>> = Box::new(MinimalSubSinkInletHandler { pushes: 0 });

  // Then: 型消去した handler として保持可能
  let _ = handler;
}
