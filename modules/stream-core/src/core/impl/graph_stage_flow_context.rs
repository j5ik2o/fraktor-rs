use alloc::{boxed::Box, vec::Vec};

use fraktor_actor_core_rs::system::ActorSystem;

use crate::core::{
  DynValue, StreamError,
  stage::{AsyncCallback, StageActor, StageActorReceive, StageContext, TimerGraphStageLogic},
};

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
  actor_system:         Option<ActorSystem>,
  stage_actor:          Option<StageActor>,
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
      actor_system:  None,
      stage_actor:   None,
    }
  }

  pub(crate) fn set_actor_system(&mut self, system: ActorSystem) {
    self.actor_system = Some(system);
  }

  /// Sets the current input element (called by the adapter before `on_push`).
  pub(crate) fn set_input(&mut self, input: In) {
    self.pulled = false;
    self.input = Some(input);
  }

  /// Takes all buffered output values, leaving the buffer empty.
  pub(crate) fn take_outputs(&mut self) -> Vec<DynValue> {
    core::mem::take(&mut self.outputs)
  }

  /// Takes the stored failure, if any, leaving `None`.
  pub(crate) fn take_failure(&mut self) -> Option<StreamError> {
    let failed = self.failed.clone();
    self.failed = None;
    failed
  }

  pub(crate) const fn take_completed(&mut self) -> bool {
    let completed = self.completed;
    self.completed = false;
    completed
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

  pub(crate) fn drain_stage_actor_messages(&self) -> Result<(), StreamError> {
    if let Some(stage_actor) = &self.stage_actor {
      stage_actor.drain_pending()?;
    }
    Ok(())
  }

  pub(crate) fn stop_stage_actor(&mut self) -> Result<(), StreamError> {
    if let Some(stage_actor) = self.stage_actor.take() {
      stage_actor.stop()?;
    }
    Ok(())
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

  fn get_stage_actor(&mut self, receive: Box<dyn StageActorReceive>) -> Result<StageActor, StreamError> {
    if let Some(stage_actor) = &self.stage_actor {
      stage_actor.r#become(receive);
      return Ok(stage_actor.clone());
    }
    let system = self.actor_system.as_ref().ok_or(StreamError::ActorSystemMissing)?;
    let stage_actor = StageActor::new(system, receive);
    self.stage_actor = Some(stage_actor.clone());
    Ok(stage_actor)
  }

  fn stage_actor(&self) -> Result<StageActor, StreamError> {
    self.stage_actor.clone().ok_or(StreamError::StageActorRefNotInitialized)
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
