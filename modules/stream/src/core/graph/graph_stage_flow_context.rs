use alloc::{boxed::Box, vec::Vec};

use super::{DynValue, StageContext, StreamError};
use crate::core::stage::{AsyncCallback, TimerGraphStageLogic};

#[cfg(test)]
mod tests;

/// Concrete `StageContext` implementation that bridges `GraphStageLogic` callbacks
/// to the interpreter's `FlowLogic` protocol.
///
/// Buffers push/pull/complete/fail calls from user logic so that
/// `GraphStageFlowAdapter` can translate them into `Vec<DynValue>` and error results.
pub(crate) struct GraphStageFlowContext<In, Out> {
  input:                Option<In>,
  outputs:              Vec<DynValue>,
  pulled:               bool,
  pub(crate) completed: bool,
  failed:               Option<StreamError>,
  input_closed:         bool,
  output_closed:        bool,
  async_cb:             AsyncCallback<Out>,
  timer:                TimerGraphStageLogic,
}

impl<In, Out> GraphStageFlowContext<In, Out>
where
  Out: Send + 'static,
{
  /// Creates a new context with all state reset.
  pub(crate) fn new() -> Self {
    Self {
      input:         None,
      outputs:       Vec::new(),
      pulled:        false,
      completed:     false,
      failed:        None,
      input_closed:  false,
      output_closed: false,
      async_cb:      AsyncCallback::new(),
      timer:         TimerGraphStageLogic::new(),
    }
  }

  /// Sets the current input element (called by the adapter before `on_push`).
  pub(crate) fn set_input(&mut self, input: In) {
    self.input = Some(input);
  }

  /// Takes all buffered output values, leaving the buffer empty.
  pub(crate) fn take_outputs(&mut self) -> Vec<DynValue> {
    core::mem::take(&mut self.outputs)
  }

  /// Takes the stored failure, if any, leaving `None`.
  pub(crate) const fn take_failure(&mut self) -> Option<StreamError> {
    self.failed.take()
  }

  /// Marks the input port as closed.
  pub(crate) const fn mark_input_closed(&mut self) {
    self.input_closed = true;
  }

  /// Marks the output port as closed.
  pub(crate) const fn mark_output_closed(&mut self) {
    self.output_closed = true;
  }

  /// Returns `true` if there are buffered output values waiting to be drained.
  pub(crate) fn has_outputs(&self) -> bool {
    !self.outputs.is_empty()
  }
}

impl<In, Out> StageContext<In, Out> for GraphStageFlowContext<In, Out>
where
  Out: Send + 'static,
{
  fn pull(&mut self) {
    self.pulled = true;
  }

  fn grab(&mut self) -> In {
    // Safety invariant: the adapter always calls set_input before on_push.
    // The #[should_panic] test verifies this panic path for misuse detection.
    #[allow(clippy::expect_used)]
    self.input.take().expect("grab called without available input")
  }

  fn push(&mut self, out: Out) {
    self.outputs.push(Box::new(out) as DynValue);
  }

  fn complete(&mut self) {
    self.completed = true;
  }

  fn fail(&mut self, error: StreamError) {
    self.failed = Some(error);
  }

  fn async_callback(&self) -> &AsyncCallback<Out> {
    &self.async_cb
  }

  fn timer_graph_stage_logic(&mut self) -> &mut TimerGraphStageLogic {
    &mut self.timer
  }

  fn has_been_pulled(&self) -> bool {
    self.pulled
  }

  fn is_available(&self) -> bool {
    self.input.is_some()
  }

  fn is_closed_in(&self) -> bool {
    self.input_closed
  }

  fn is_closed_out(&self) -> bool {
    self.output_closed
  }
}
