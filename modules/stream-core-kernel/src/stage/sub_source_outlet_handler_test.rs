use alloc::boxed::Box;

use super::SubSourceOutletHandler;
use crate::{StreamError, stage::CancellationCause};

struct MinimalSubSourceOutletHandler {
  pulls: usize,
}

impl SubSourceOutletHandler<u32> for MinimalSubSourceOutletHandler {
  fn on_pull(&mut self) -> Result<(), StreamError> {
    self.pulls = self.pulls.saturating_add(1);
    Ok(())
  }
}

struct CapturingSubSourceOutletHandler {
  last_cause: Option<CancellationCause>,
}

impl SubSourceOutletHandler<u32> for CapturingSubSourceOutletHandler {
  fn on_pull(&mut self) -> Result<(), StreamError> {
    Ok(())
  }

  fn on_downstream_finish(&mut self, cause: CancellationCause) -> Result<(), StreamError> {
    self.last_cause = Some(cause);
    Ok(())
  }
}

#[test]
fn trait_can_be_implemented_with_only_on_pull() {
  // Given: on_pull だけを明示実装する handler
  let mut handler = MinimalSubSourceOutletHandler { pulls: 0 };

  // When: pull callback を呼び出す
  let result = handler.on_pull();

  // Then: downstream finish は default 実装に任せられる
  assert_eq!(result, Ok(()));
  assert_eq!(handler.pulls, 1);
  assert_eq!(handler.on_downstream_finish(CancellationCause::no_more_elements_needed()), Ok(()));
}

#[test]
fn trait_can_capture_downstream_finish_cause_when_overridden() {
  // Given: cancellation cause を記録する handler
  let mut handler = CapturingSubSourceOutletHandler { last_cause: None };

  // When: downstream finish callback を呼び出す
  let result = handler.on_downstream_finish(CancellationCause::stage_was_completed());

  // Then: Pekko の OutHandler と同じく cancellation cause を handler 側で扱える
  assert_eq!(result, Ok(()));
  assert_eq!(handler.last_cause, Some(CancellationCause::stage_was_completed()));
}

#[test]
fn handler_is_usable_as_trait_object() {
  // Given: 動的 substream port が handler を trait object として保持できる
  let handler: Box<dyn SubSourceOutletHandler<u32>> = Box::new(MinimalSubSourceOutletHandler { pulls: 0 });

  // Then: 型消去した handler として保持可能
  let _ = handler;
}
