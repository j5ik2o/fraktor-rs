use alloc::{boxed::Box, vec::Vec};
use core::any::Any;

use super::{
  DynValue, StageContext, StreamError, graph_stage_flow_context::GraphStageFlowContext,
  graph_stage_logic::GraphStageLogic,
};
use crate::core::{DownstreamCancelAction, FailureAction, FlowLogic};

#[cfg(test)]
mod tests;

/// Adapter that bridges a `GraphStageLogic` into the interpreter's `FlowLogic` protocol.
///
/// On each `apply()` call the adapter:
/// 1. Downcasts the `DynValue` to the typed `In`
/// 2. Calls `on_start` on the first invocation
/// 3. Sets the input on the context and calls `on_push`
/// 4. Collects outputs or failures from the context
pub(crate) struct GraphStageFlowAdapter<In, Out, Mat> {
  logic:   Box<dyn GraphStageLogic<In, Out, Mat> + Send>,
  context: GraphStageFlowContext<In, Out>,
  started: bool,
}

impl<In, Out, Mat> GraphStageFlowAdapter<In, Out, Mat>
where
  In: Any + Send + 'static,
  Out: Send + 'static,
  Mat: Send + 'static,
{
  /// Creates a new adapter wrapping the given stage logic.
  pub(crate) fn new(logic: Box<dyn GraphStageLogic<In, Out, Mat> + Send>) -> Self {
    Self { logic, context: GraphStageFlowContext::new(), started: false }
  }
}

impl<In, Out, Mat> GraphStageFlowAdapter<In, Out, Mat>
where
  In: Any + Send + 'static,
  Out: Send + 'static,
  Mat: Send + 'static,
{
  /// Ensures `on_start` is called exactly once.
  fn ensure_started(&mut self) {
    if !self.started {
      self.started = true;
      self.logic.on_start(&mut self.context);
    }
  }
}

impl<In, Out, Mat> FlowLogic for GraphStageFlowAdapter<In, Out, Mat>
where
  In: Any + Send + 'static,
  Out: Send + 'static,
  Mat: Send + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let typed_input = input.downcast::<In>().map(|b| *b).map_err(|_| StreamError::TypeMismatch)?;

    self.ensure_started();

    self.context.set_input(typed_input);
    self.logic.on_push(&mut self.context);

    if let Some(err) = self.context.take_failure() {
      return Err(err);
    }

    Ok(self.context.take_outputs())
  }

  fn handles_failures(&self) -> bool {
    true
  }

  fn on_failure(&mut self, error: StreamError) -> Result<FailureAction, StreamError> {
    self.ensure_started();
    self.logic.on_error(&mut self.context, error);
    if let Some(err) = self.context.take_failure() {
      return Ok(FailureAction::Propagate(err));
    }
    Ok(FailureAction::Resume)
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    self.ensure_started();
    self.context.mark_input_closed();
    self.logic.on_complete(&mut self.context);
    self.logic.on_stop(&mut self.context);
    if let Some(err) = self.context.take_failure() {
      return Err(err);
    }
    Ok(())
  }

  fn on_downstream_cancel(&mut self) -> Result<DownstreamCancelAction, StreamError> {
    self.ensure_started();
    self.context.mark_output_closed();
    self.logic.on_stop(&mut self.context);
    Ok(DownstreamCancelAction::Propagate)
  }

  fn on_async_callback(&mut self) -> Result<Vec<DynValue>, StreamError> {
    self.logic.on_async_callback(&mut self.context);
    if let Some(err) = self.context.take_failure() {
      return Err(err);
    }
    Ok(self.context.take_outputs())
  }

  fn on_timer(&mut self) -> Result<Vec<DynValue>, StreamError> {
    let fired_keys = self.context.advance_timers();
    for key in fired_keys {
      self.logic.on_timer(&mut self.context, key);
      if let Some(err) = self.context.take_failure() {
        return Err(err);
      }
    }
    Ok(self.context.take_outputs())
  }

  fn on_tick(&mut self, _tick_count: u64) -> Result<(), StreamError> {
    // Timer advancement is handled in on_timer()
    Ok(())
  }

  fn has_pending_output(&self) -> bool {
    self.context.has_outputs()
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    if let Some(err) = self.context.take_failure() {
      return Err(err);
    }
    Ok(self.context.take_outputs())
  }
}
