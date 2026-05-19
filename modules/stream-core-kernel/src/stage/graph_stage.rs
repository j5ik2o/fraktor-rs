use alloc::boxed::Box;

use super::graph_stage_logic::GraphStageLogic;
use crate::shape::StreamShape;

/// Graph stage definition.
pub trait GraphStage<In, Out, Mat> {
  /// Returns the stage shape.
  fn shape(&self) -> StreamShape<In, Out>;
  /// Creates the stage logic instance.
  fn create_logic(&self) -> Box<dyn GraphStageLogic<In, Out, Mat> + Send>;
}
