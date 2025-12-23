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

  /// Returns the materialized value for this stage.
  fn materialized(&mut self) -> Mat;
}
