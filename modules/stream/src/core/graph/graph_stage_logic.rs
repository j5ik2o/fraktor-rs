use super::{StageContext, StreamError};

/// Processing logic for a graph stage.
pub trait GraphStageLogic<In, Out, Mat> {
  /// Called when the stage starts.
  fn on_start(&mut self, _ctx: &mut dyn StageContext<In, Out>) {}

  /// Called when downstream requests demand.
  fn on_pull(&mut self, _ctx: &mut dyn StageContext<In, Out>) {}

  /// Called when an element is available.
  fn on_push(&mut self, _ctx: &mut dyn StageContext<In, Out>) {}

  /// Called when upstream completes.
  fn on_complete(&mut self, _ctx: &mut dyn StageContext<In, Out>) {}

  /// Called when upstream fails.
  fn on_error(&mut self, _ctx: &mut dyn StageContext<In, Out>, _error: StreamError) {}

  /// Called when asynchronous callbacks are ready to be drained.
  fn on_async_callback(&mut self, _ctx: &mut dyn StageContext<In, Out>) {}

  /// Called when a timer fires.
  fn on_timer(&mut self, _ctx: &mut dyn StageContext<In, Out>, _timer_key: u64) {}

  /// Returns the materialized value for this stage.
  fn materialized(&mut self) -> Mat;
}
