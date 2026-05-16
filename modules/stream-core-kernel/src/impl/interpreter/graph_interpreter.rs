use alloc::{vec, vec::Vec};
use core::any::TypeId;

mod failure_restart;
#[cfg(test)]
#[path = "graph_interpreter_test.rs"]
mod tests;

use fraktor_actor_core_kernel_rs::system::ActorSystem;

use super::{
  compiled_graph_plan::CompiledGraphPlan, failure_disposition::FailureDisposition, graph_connections::GraphConnections,
  interpreter_snapshot_builder::InterpreterSnapshotBuilder,
};
use crate::{
  DownstreamCancelAction, DynValue, SinkDecision, StageDefinition, StreamError, StreamPlan,
  r#impl::{
    fusing::{DemandTracker, StreamBufferConfig},
    materialization::StreamState,
  },
  materialization::DriveOutcome,
  shape::PortId,
  snapshot::RunningInterpreter,
  stream_ref::StreamRefSettings,
};

struct FlowStagePorts {
  inlet:       PortId,
  outlet:      PortId,
  input_type:  TypeId,
  output_type: TypeId,
}

#[derive(Clone, Copy)]
struct SourceStagePorts {
  outlet:      PortId,
  output_type: TypeId,
}

#[derive(Default)]
struct FlowStageStep {
  outputs:          Vec<DynValue>,
  consumed_input:   bool,
  skip_stage_input: bool,
  force_shutdown:   bool,
  progressed:       bool,
}

#[derive(Clone, Copy)]
struct SinkStagePorts {
  inlet:      PortId,
  input_type: TypeId,
}

enum SinkPushOutcome {
  Decision(SinkDecision),
  Progressed,
}

/// Executes a stream graph using a port-driven runtime.
pub(crate) struct GraphInterpreter {
  stages:                 Vec<StageDefinition>,
  connections:            GraphConnections,
  flow_order:             Vec<usize>,
  flow_slots:             Vec<Option<usize>>,
  source_indices:         Vec<usize>,
  sink_indices:           Vec<usize>,
  demand:                 DemandTracker,
  state:                  StreamState,
  source_done:            Vec<bool>,
  source_canceled:        Vec<bool>,
  source_shutdown:        Vec<bool>,
  sink_done:              Vec<bool>,
  sink_started:           Vec<bool>,
  flow_source_done:       Vec<bool>,
  flow_done:              Vec<bool>,
  sink_upstream_notified: Vec<bool>,
  on_start_done:          bool,
  tick_count:             u64,
}

impl GraphInterpreter {
  /// Creates a new interpreter from the provided plan.
  ///
  /// # Panics
  ///
  /// Panics when the provided plan is structurally invalid.
  #[must_use]
  pub(crate) fn new(plan: StreamPlan, buffer_config: StreamBufferConfig) -> Self {
    Self::new_with_materializer_context(plan, buffer_config, None, &StreamRefSettings::new())
  }

  pub(crate) fn new_with_materializer_context(
    plan: StreamPlan,
    buffer_config: StreamBufferConfig,
    actor_system: Option<&ActorSystem>,
    stream_ref_settings: &StreamRefSettings,
  ) -> Self {
    let compiled = CompiledGraphPlan::compile(plan, buffer_config);
    let mut stages = compiled.stages;
    Self::attach_materializer_context(&mut stages, actor_system, stream_ref_settings);
    let stage_count = stages.len();
    let flow_count = compiled.flow_order.len();
    let source_indices_len = compiled.source_indices.len();
    let sink_indices_len = compiled.sink_indices.len();
    let mut flow_slots = vec![None; stage_count];
    for (flow_slot, stage_index) in compiled.flow_order.iter().copied().enumerate() {
      flow_slots[stage_index] = Some(flow_slot);
    }
    Self {
      stages,
      connections: GraphConnections::new(compiled.edges, compiled.dispatch),
      flow_order: compiled.flow_order,
      flow_slots,
      source_indices: compiled.source_indices,
      sink_indices: compiled.sink_indices,
      demand: DemandTracker::new(),
      state: StreamState::Idle,
      source_done: vec![false; source_indices_len],
      source_canceled: vec![false; source_indices_len],
      source_shutdown: vec![false; source_indices_len],
      sink_done: vec![false; sink_indices_len],
      sink_started: vec![false; sink_indices_len],
      flow_source_done: vec![false; flow_count],
      flow_done: vec![false; flow_count],
      sink_upstream_notified: vec![false; sink_indices_len],
      on_start_done: false,
      tick_count: 0,
    }
  }

  fn attach_materializer_context(
    stages: &mut [StageDefinition],
    actor_system: Option<&ActorSystem>,
    stream_ref_settings: &StreamRefSettings,
  ) {
    for stage in stages {
      match stage {
        | StageDefinition::Source(source) => source.logic.attach_stream_ref_settings(stream_ref_settings.clone()),
        | StageDefinition::Flow(flow) => {
          if let Some(system) = actor_system {
            flow.logic.attach_actor_system(system.clone());
          }
        },
        | StageDefinition::Sink(sink) => sink.logic.attach_stream_ref_settings(stream_ref_settings.clone()),
      };
    }
  }

  /// Starts the interpreter.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when the sink cannot start.
  pub(crate) fn start(&mut self) -> Result<(), StreamError> {
    if self.state != StreamState::Idle {
      return Ok(());
    }
    self.state = StreamState::Running;
    if !self.on_start_done {
      self.start_sinks()?;
      self.on_start_done = true;
    }
    Ok(())
  }

  /// Returns the current stream state.
  #[must_use]
  pub(crate) const fn state(&self) -> StreamState {
    self.state
  }

  /// Builds a diagnostic snapshot of this interpreter.
  ///
  /// Corresponds to Pekko `GraphInterpreterShell.toSnapshot` when the
  /// interpreter is in the running phase: collects one [`LogicSnapshot`] per
  /// stage, derives [`ConnectionSnapshot`]s from the current edge runtime, and
  /// reports the number of logics still alive.
  #[must_use]
  pub(crate) fn snapshot(&self) -> RunningInterpreter {
    InterpreterSnapshotBuilder::new(&self.stages).build(self.connections.edges())
  }

  /// Cancels the stream.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when cancellation cannot be processed.
  pub(crate) fn cancel(&mut self) -> Result<(), StreamError> {
    if self.state.is_terminal() {
      return Ok(());
    }
    self.cancel_source_if_needed()?;
    self.set_all_sources_done()?;
    self.state = StreamState::Cancelled;
    Ok(())
  }

  pub(crate) fn request_shutdown(&mut self) -> Result<(), StreamError> {
    if self.state.is_terminal() {
      return Ok(());
    }
    if self.state == StreamState::Idle
      && let Err(error) = self.start()
    {
      self.fail(&error);
      return Err(error);
    }
    self.shutdown_sources_if_needed()?;
    Ok(())
  }

  pub(crate) fn abort(&mut self, error: &StreamError) {
    if self.state.is_terminal() {
      return;
    }
    if let Err(cancel_error) = self.cancel_source_if_needed() {
      self.fail(&cancel_error);
      return;
    }
    if let Err(error) = self.set_all_sources_done() {
      self.fail(&error);
      return;
    }
    self.fail(error);
  }

  /// Drives the interpreter once.
  #[must_use]
  pub(crate) fn drive(&mut self) -> DriveOutcome {
    if self.state != StreamState::Running {
      return DriveOutcome::Idle;
    }

    match self.drive_running() {
      | Ok(true) => DriveOutcome::Progressed,
      | Ok(false) => DriveOutcome::Idle,
      | Err(error) => {
        self.fail(&error);
        DriveOutcome::Progressed
      },
    }
  }

  fn drive_running(&mut self) -> Result<bool, StreamError> {
    self.tick_count = self.tick_count.saturating_add(1);
    self.tick_restart_windows()?;

    let mut progressed = self.tick_flow_stages()?;
    progressed |= self.start_sinks_if_needed()?;

    if self.state != StreamState::Running {
      return Ok(true);
    }

    progressed |= self.pull_and_drive_flows_if_needed()?;
    progressed |= self.complete_if_runtime_work_done();
    progressed |= self.drive_sinks_once()?;
    progressed |= self.finish_sources_done_stream()?;

    Ok(progressed)
  }

  fn start_sinks_if_needed(&mut self) -> Result<bool, StreamError> {
    if self.on_start_done {
      return Ok(false);
    }
    self.start_sinks()?;
    self.on_start_done = true;
    Ok(true)
  }

  fn pull_and_drive_flows_if_needed(&mut self) -> Result<bool, StreamError> {
    let did_pull = if self.demand.has_demand() {
      self.pull_sources_if_needed()?
    } else if self.has_flow_requesting_upstream_drain() {
      self.pull_sources_for_flows_requesting_drain()?
    } else {
      return Ok(false);
    };
    let drove_flows = self.drive_flow_stages_until_idle()?;
    Ok(did_pull || drove_flows)
  }

  fn drive_flow_stages_until_idle(&mut self) -> Result<bool, StreamError> {
    let mut progressed = false;
    while self.drive_flow_stages_once()? {
      progressed = true;
    }
    Ok(progressed)
  }

  fn complete_if_runtime_work_done(&mut self) -> bool {
    if !self.runtime_work_done() {
      return false;
    }
    self.state = StreamState::Completed;
    true
  }

  fn runtime_work_done(&self) -> bool {
    self.state == StreamState::Running
      && self.all_sinks_done()
      && self.all_sources_done()
      && !self.restart_waiting()
      && !self.has_flow_requesting_upstream_drain()
      && self.all_edge_buffers_empty()
      && !self.flow_order.iter().any(|stage_index| self.flow_has_pending_output(*stage_index))
  }

  fn restart_waiting(&self) -> bool {
    self.source_restart_waiting()
      || self.sink_restart_waiting()
      || self.flow_order.iter().any(|stage_index| self.flow_restart_waiting(*stage_index))
  }

  fn finish_sources_done_stream(&mut self) -> Result<bool, StreamError> {
    if !self.sources_done_stream_ready_to_finish() {
      return Ok(false);
    }

    let mut progressed = self.drive_flow_stages_until_idle()?;
    if self.all_edge_buffers_empty() {
      progressed |= self.finish_sinks()?;
      if self.all_sinks_done() {
        self.state = StreamState::Completed;
        progressed = true;
      }
    }
    Ok(progressed)
  }

  fn sources_done_stream_ready_to_finish(&self) -> bool {
    self.all_sources_done()
      && self.state == StreamState::Running
      && !self.restart_waiting()
      && !self.flow_order.iter().any(|stage_index| self.flow_has_pending_output(*stage_index))
  }

  fn tick_flow_stages(&mut self) -> Result<bool, StreamError> {
    let mut progressed = false;

    for flow_index in 0..self.flow_order.len() {
      let stage_index = self.flow_order[flow_index];
      if self.flow_done_at(stage_index) {
        continue;
      }
      if self.flow_restart_waiting(stage_index) {
        continue;
      }
      if self.mark_flow_source_done(stage_index)? {
        progressed = true;
      }

      let on_tick_result = {
        let StageDefinition::Flow(flow) = &mut self.stages[stage_index] else {
          continue;
        };
        flow.logic.on_tick(self.tick_count)
      };
      match on_tick_result {
        | Ok(()) => {},
        | Err(error) => match self.handle_flow_failure(stage_index, &error)? {
          | FailureDisposition::Continue => {
            progressed = true;
            continue;
          },
          | FailureDisposition::Complete => {
            if !self.all_sources_done() {
              self.set_all_sources_done()?;
            }
            self.shutdown_flow_stage(stage_index)?;
            self.maybe_finish_flow_stage(stage_index);
            progressed = true;
            continue;
          },
          | FailureDisposition::Fail(error) => return Err(error),
        },
      }

      let shutdown_requested = {
        let StageDefinition::Flow(flow) = &mut self.stages[stage_index] else {
          continue;
        };
        flow.logic.take_shutdown_request()
      };
      if shutdown_requested {
        self.shutdown_flow_stage(stage_index)?;
        progressed = true;
      }
      if self.maybe_finish_flow_stage(stage_index) {
        progressed = true;
      }
    }

    Ok(progressed)
  }

  fn start_sinks(&mut self) -> Result<(), StreamError> {
    for sink_position in 0..self.sink_indices.len() {
      if self.sink_started[sink_position] {
        continue;
      }
      let sink_index = self.sink_indices[sink_position];
      let on_start_result = {
        let StageDefinition::Sink(sink) = &mut self.stages[sink_index] else {
          return Err(StreamError::InvalidConnection);
        };
        sink.logic.on_start(&mut self.demand)
      };
      self.sink_started[sink_position] = true;
      on_start_result?;
    }
    Ok(())
  }

  fn notify_source_done_to_flows(&mut self) -> Result<(), StreamError> {
    for flow_index in 0..self.flow_order.len() {
      let stage_index = self.flow_order[flow_index];
      self.mark_flow_source_done(stage_index)?;
    }
    Ok(())
  }

  fn cancel_source_if_needed(&mut self) -> Result<(), StreamError> {
    for source_position in 0..self.source_indices.len() {
      if self.source_done[source_position] || self.source_canceled[source_position] {
        continue;
      }
      let source_index = self.source_indices[source_position];
      let on_cancel_result = {
        let StageDefinition::Source(source) = &mut self.stages[source_index] else {
          return Err(StreamError::InvalidConnection);
        };
        source.logic.on_cancel()
      };
      self.source_canceled[source_position] = true;
      on_cancel_result?;
    }
    Ok(())
  }

  fn shutdown_sources_if_needed(&mut self) -> Result<(), StreamError> {
    let mut source_done_changed = false;
    for source_position in 0..self.source_indices.len() {
      if self.source_done[source_position]
        || self.source_canceled[source_position]
        || self.source_shutdown[source_position]
      {
        continue;
      }
      let source_index = self.source_indices[source_position];
      let (should_drain_on_shutdown, on_shutdown_result, on_cancel_result_opt) = {
        let StageDefinition::Source(source) = &mut self.stages[source_index] else {
          return Err(StreamError::InvalidConnection);
        };
        let should_drain = source.logic.should_drain_on_shutdown();
        let on_shutdown_result = source.logic.on_shutdown();
        let on_cancel_result_opt = match (&on_shutdown_result, should_drain) {
          | (Ok(()), false) => Some(source.logic.on_cancel()),
          | _ => None,
        };
        (should_drain, on_shutdown_result, on_cancel_result_opt)
      };
      self.source_shutdown[source_position] = true;
      on_shutdown_result?;
      if should_drain_on_shutdown {
        continue;
      }
      self.source_done[source_position] = true;
      self.source_canceled[source_position] = true;
      on_cancel_result_opt.transpose()?;
      self.close_outgoing_edges_for_stage(source_index);
      source_done_changed = true;
    }
    if source_done_changed {
      self.notify_source_done_to_flows()?;
    }
    Ok(())
  }

  fn pull_sources_if_needed(&mut self) -> Result<bool, StreamError> {
    let source_positions: Vec<usize> = (0..self.source_indices.len()).collect();
    self.pull_source_positions_if_needed(&source_positions)
  }

  fn pull_sources_for_flows_requesting_drain(&mut self) -> Result<bool, StreamError> {
    let source_positions = self.source_positions_for_flows_requesting_drain();
    if source_positions.is_empty() {
      return Ok(false);
    }
    self.pull_source_positions_if_needed(&source_positions)
  }

  fn pull_source_positions_if_needed(&mut self, source_positions: &[usize]) -> Result<bool, StreamError> {
    let mut progressed = false;

    for &source_position in source_positions {
      progressed |= self.pull_source_position_if_needed(source_position)?;
    }

    Ok(progressed)
  }

  fn pull_source_position_if_needed(&mut self, source_position: usize) -> Result<bool, StreamError> {
    if !self.source_ready_to_pull(source_position) {
      return Ok(false);
    }

    let source_index = self.source_indices[source_position];
    let ports = self.source_stage_ports(source_index)?;
    if self.has_buffered_outgoing(ports.outlet) {
      return Ok(false);
    }

    match self.pull_source_value(source_index) {
      | Ok(Some(value)) => self.emit_source_value(ports, value),
      | Ok(None) => self.complete_or_restart_source(source_position, source_index),
      | Err(StreamError::WouldBlock) => Ok(false),
      | Err(error) => self.apply_source_failure(source_position, error),
    }
  }

  fn source_ready_to_pull(&self, source_position: usize) -> bool {
    !self.source_done[source_position] && !self.source_restart_waiting_at(source_position)
  }

  fn source_stage_ports(&self, source_index: usize) -> Result<SourceStagePorts, StreamError> {
    match &self.stages[source_index] {
      | StageDefinition::Source(source) => {
        Ok(SourceStagePorts { outlet: source.outlet, output_type: source.output_type })
      },
      | _ => Err(StreamError::InvalidConnection),
    }
  }

  fn pull_source_value(&mut self, source_index: usize) -> Result<Option<DynValue>, StreamError> {
    let StageDefinition::Source(source) = &mut self.stages[source_index] else {
      return Err(StreamError::InvalidConnection);
    };
    source.logic.pull()
  }

  fn emit_source_value(&mut self, ports: SourceStagePorts, value: DynValue) -> Result<bool, StreamError> {
    if value.as_ref().type_id() != ports.output_type {
      return Err(StreamError::TypeMismatch);
    }
    self.offer_to_next_outgoing_edge(ports.outlet, value)?;
    Ok(true)
  }

  fn complete_or_restart_source(&mut self, source_position: usize, source_index: usize) -> Result<bool, StreamError> {
    let (should_restart, complete_on_exhaustion) = self.schedule_source_restart_or_complete(source_index)?;
    if should_restart {
      return Ok(true);
    }
    if !complete_on_exhaustion {
      return Err(StreamError::Failed);
    }
    self.complete_source(source_position)?;
    Ok(true)
  }

  fn schedule_source_restart_or_complete(&mut self, source_index: usize) -> Result<(bool, bool), StreamError> {
    let StageDefinition::Source(source) = &mut self.stages[source_index] else {
      return Err(StreamError::InvalidConnection);
    };
    Ok(
      if let Some(restart) = &mut source.restart {
        (restart.schedule(self.tick_count), restart.complete_on_max_restarts())
      } else {
        (false, true)
      },
    )
  }

  fn apply_source_failure(&mut self, source_position: usize, error: StreamError) -> Result<bool, StreamError> {
    match self.handle_source_failure(source_position, error)? {
      | FailureDisposition::Continue => Ok(true),
      | FailureDisposition::Complete => {
        self.complete_source(source_position)?;
        Ok(true)
      },
      | FailureDisposition::Fail(error) => Err(error),
    }
  }

  fn drive_flow_stages_once(&mut self) -> Result<bool, StreamError> {
    let mut progressed = false;

    for flow_index in 0..self.flow_order.len() {
      let stage_index = self.flow_order[flow_index];
      if self.drive_flow_stage_once(stage_index)? {
        progressed = true;
      }
    }

    Ok(progressed)
  }

  fn drive_flow_stage_once(&mut self, stage_index: usize) -> Result<bool, StreamError> {
    if self.flow_stage_inactive(stage_index) {
      return Ok(false);
    }

    let source_progressed = self.mark_flow_source_done(stage_index)?;
    let Some(ports) = self.flow_stage_ports(stage_index) else {
      return Ok(source_progressed);
    };
    let outgoing_buffered = self.has_buffered_outgoing(ports.outlet);
    if self.flow_stage_output_backpressured(stage_index, outgoing_buffered) {
      return Ok(source_progressed);
    }

    let stage_progressed = self.drive_ready_flow_stage_once(stage_index, &ports, outgoing_buffered)?;
    Ok(source_progressed || stage_progressed)
  }

  fn flow_stage_inactive(&self, stage_index: usize) -> bool {
    self.flow_done_at(stage_index) || self.flow_restart_waiting(stage_index)
  }

  fn flow_stage_output_backpressured(&self, stage_index: usize, outgoing_buffered: bool) -> bool {
    outgoing_buffered && !self.flow_can_accept_input_while_output_buffered(stage_index)
  }

  fn drive_ready_flow_stage_once(
    &mut self,
    stage_index: usize,
    ports: &FlowStagePorts,
    outgoing_buffered: bool,
  ) -> Result<bool, StreamError> {
    let mut step = FlowStageStep::default();
    self.append_flow_async_outputs(stage_index, &mut step)?;
    self.append_flow_timer_outputs(stage_index, &mut step)?;
    self.apply_flow_stage_input_if_ready(stage_index, ports, &mut step)?;
    if self.drain_flow_stage_pending_if_ready(stage_index, outgoing_buffered, &mut step)? {
      return Ok(true);
    }

    let progressed = step.consumed_input || step.progressed;
    let shutdown_requested = self.take_flow_shutdown_request(stage_index)?;
    if step.outputs.is_empty() {
      let finished = self.finish_flow_stage_without_outputs(stage_index, shutdown_requested, step.force_shutdown)?;
      return Ok(progressed || finished);
    }

    self.emit_flow_stage_outputs(stage_index, ports, step.outputs)?;
    if shutdown_requested || step.force_shutdown {
      self.shutdown_flow_stage(stage_index)?;
    }
    self.maybe_finish_flow_stage(stage_index);
    Ok(true)
  }

  fn flow_stage_ports(&self, stage_index: usize) -> Option<FlowStagePorts> {
    match &self.stages[stage_index] {
      | StageDefinition::Flow(flow) => Some(FlowStagePorts {
        inlet:       flow.inlet,
        outlet:      flow.outlet,
        input_type:  flow.input_type,
        output_type: flow.output_type,
      }),
      | _ => None,
    }
  }

  fn flow_can_accept_input_while_output_buffered(&self, stage_index: usize) -> bool {
    match &self.stages[stage_index] {
      | StageDefinition::Flow(flow) => flow.logic.can_accept_input_while_output_buffered(),
      | _ => false,
    }
  }

  fn append_flow_async_outputs(&mut self, stage_index: usize, step: &mut FlowStageStep) -> Result<(), StreamError> {
    let async_outputs = {
      let StageDefinition::Flow(flow) = &mut self.stages[stage_index] else {
        return Err(StreamError::InvalidConnection);
      };
      flow.logic.on_async_callback()
    };
    self.append_flow_hook_outputs(stage_index, async_outputs, step)
  }

  fn append_flow_timer_outputs(&mut self, stage_index: usize, step: &mut FlowStageStep) -> Result<(), StreamError> {
    let timer_outputs = {
      let StageDefinition::Flow(flow) = &mut self.stages[stage_index] else {
        return Err(StreamError::InvalidConnection);
      };
      flow.logic.on_timer()
    };
    self.append_flow_hook_outputs(stage_index, timer_outputs, step)
  }

  fn append_flow_hook_outputs(
    &mut self,
    stage_index: usize,
    hook_outputs: Result<Vec<DynValue>, StreamError>,
    step: &mut FlowStageStep,
  ) -> Result<(), StreamError> {
    match hook_outputs {
      | Ok(outputs) => {
        step.outputs.extend(outputs);
        Ok(())
      },
      | Err(error) => self.apply_flow_failure_to_step(stage_index, &error, step),
    }
  }

  fn apply_flow_failure_to_step(
    &mut self,
    stage_index: usize,
    error: &StreamError,
    step: &mut FlowStageStep,
  ) -> Result<(), StreamError> {
    match self.handle_flow_failure(stage_index, error)? {
      | FailureDisposition::Continue => {
        step.progressed = true;
        step.skip_stage_input = true;
        Ok(())
      },
      | FailureDisposition::Complete => {
        self.set_all_sources_done()?;
        step.progressed = true;
        step.skip_stage_input = true;
        step.force_shutdown = true;
        Ok(())
      },
      | FailureDisposition::Fail(error) => Err(error),
    }
  }

  fn apply_flow_stage_input_if_ready(
    &mut self,
    stage_index: usize,
    ports: &FlowStagePorts,
    step: &mut FlowStageStep,
  ) -> Result<(), StreamError> {
    if !self.flow_can_accept_input(stage_index, step.skip_stage_input) {
      return Ok(());
    }

    let preferred_input_slot = self.flow_preferred_input_edge_slot(stage_index);
    let Some((edge_index, input)) = self.poll_from_incoming_edges(ports.inlet, preferred_input_slot)? else {
      return Ok(());
    };
    step.consumed_input = true;
    if input.as_ref().type_id() != ports.input_type {
      return Err(StreamError::TypeMismatch);
    }

    let apply_result = self.apply_flow_logic_with_edge(stage_index, edge_index, input);
    match apply_result {
      | Ok(outputs) => {
        step.outputs.extend(outputs);
        Ok(())
      },
      | Err(error) => self.apply_flow_failure_to_step(stage_index, &error, step),
    }
  }

  fn apply_flow_logic_with_edge(
    &mut self,
    stage_index: usize,
    edge_index: usize,
    input: DynValue,
  ) -> Result<Vec<DynValue>, StreamError> {
    let StageDefinition::Flow(flow) = &mut self.stages[stage_index] else {
      return Err(StreamError::InvalidConnection);
    };
    flow.logic.apply_with_edge(edge_index, input)
  }

  fn flow_can_accept_input(&self, stage_index: usize, skip_stage_input: bool) -> bool {
    if skip_stage_input || self.flow_source_done_at(stage_index) {
      return false;
    }
    match &self.stages[stage_index] {
      | StageDefinition::Flow(flow) => flow.logic.can_accept_input(),
      | _ => false,
    }
  }

  fn flow_preferred_input_edge_slot(&self, stage_index: usize) -> Option<usize> {
    match &self.stages[stage_index] {
      | StageDefinition::Flow(flow) => flow.logic.preferred_input_edge_slot(),
      | _ => None,
    }
  }

  fn drain_flow_stage_pending_if_ready(
    &mut self,
    stage_index: usize,
    outgoing_buffered: bool,
    step: &mut FlowStageStep,
  ) -> Result<bool, StreamError> {
    // apply で新しい出力が出ず、未送信バッファもなく、この tick で input apply を明示的に
    // skip していないときだけ drain_pending に進める。
    if !step.outputs.is_empty() || outgoing_buffered || step.skip_stage_input {
      return Ok(false);
    }

    let drain_result = {
      let StageDefinition::Flow(flow) = &mut self.stages[stage_index] else {
        return Err(StreamError::InvalidConnection);
      };
      flow.logic.drain_pending()
    };
    match drain_result {
      | Ok(outputs) => {
        step.outputs = outputs;
        Ok(false)
      },
      | Err(error) => self.handle_flow_drain_failure(stage_index, &error),
    }
  }

  fn handle_flow_drain_failure(&mut self, stage_index: usize, error: &StreamError) -> Result<bool, StreamError> {
    match self.handle_flow_failure(stage_index, error)? {
      | FailureDisposition::Continue => Ok(true),
      | FailureDisposition::Complete => {
        self.set_all_sources_done()?;
        self.shutdown_flow_stage(stage_index)?;
        self.maybe_finish_flow_stage(stage_index);
        Ok(true)
      },
      | FailureDisposition::Fail(error) => Err(error),
    }
  }

  fn take_flow_shutdown_request(&mut self, stage_index: usize) -> Result<bool, StreamError> {
    let StageDefinition::Flow(flow) = &mut self.stages[stage_index] else {
      return Err(StreamError::InvalidConnection);
    };
    Ok(flow.logic.take_shutdown_request())
  }

  fn finish_flow_stage_without_outputs(
    &mut self,
    stage_index: usize,
    shutdown_requested: bool,
    force_shutdown: bool,
  ) -> Result<bool, StreamError> {
    let mut progressed = false;
    if shutdown_requested || force_shutdown {
      self.shutdown_flow_stage(stage_index)?;
      progressed = true;
    }
    if self.maybe_finish_flow_stage(stage_index) {
      progressed = true;
    }
    Ok(progressed)
  }

  fn emit_flow_stage_outputs(
    &mut self,
    stage_index: usize,
    ports: &FlowStagePorts,
    outputs: Vec<DynValue>,
  ) -> Result<(), StreamError> {
    let outgoing_edges = self.outgoing_edge_indices(ports.outlet)?;
    for output in outputs {
      if output.as_ref().type_id() != ports.output_type {
        return Err(StreamError::TypeMismatch);
      }
      match self.take_flow_next_output_edge_slot(stage_index)? {
        | Some(slot) => {
          let target = outgoing_edges[slot % outgoing_edges.len()];
          self.offer_to_outgoing_edge(target, output)?;
        },
        | None => self.offer_to_next_outgoing_edge(ports.outlet, output)?,
      }
    }
    Ok(())
  }

  fn take_flow_next_output_edge_slot(&mut self, stage_index: usize) -> Result<Option<usize>, StreamError> {
    let StageDefinition::Flow(flow) = &mut self.stages[stage_index] else {
      return Err(StreamError::InvalidConnection);
    };
    Ok(flow.logic.take_next_output_edge_slot())
  }

  fn drive_sinks_once(&mut self) -> Result<bool, StreamError> {
    let mut progressed = false;

    for sink_position in 0..self.sink_indices.len() {
      match self.drive_sink_once(sink_position)? {
        | true => progressed = true,
        | false => {},
      }
    }

    Ok(progressed)
  }

  fn drive_sink_once(&mut self, sink_position: usize) -> Result<bool, StreamError> {
    if self.sink_done[sink_position] {
      return Ok(false);
    }

    let sink_index = self.sink_indices[sink_position];
    let ports = self.sink_stage_ports(sink_index)?;
    if self.sink_restart_waiting_at(sink_index) {
      return Ok(false);
    }

    let progressed = self.tick_sink_stage(sink_position, sink_index)?;
    if !self.demand.has_demand() {
      return Ok(progressed);
    }

    if !self.sink_can_accept_input(sink_index)? {
      return self.finish_sink_if_input_exhausted(sink_position, sink_index, progressed);
    }

    let Some((_, value)) = self.poll_from_incoming_edges(ports.inlet, None)? else {
      return self.finish_sink_if_input_exhausted(sink_position, sink_index, progressed);
    };
    if value.as_ref().type_id() != ports.input_type {
      return Err(StreamError::TypeMismatch);
    }
    self.demand.consume(1)?;

    match self.push_sink_input(sink_position, sink_index, value)? {
      | SinkPushOutcome::Decision(decision) => self.apply_sink_decision(sink_position, sink_index, decision),
      | SinkPushOutcome::Progressed => Ok(true),
    }
  }

  fn sink_stage_ports(&self, sink_index: usize) -> Result<SinkStagePorts, StreamError> {
    match &self.stages[sink_index] {
      | StageDefinition::Sink(sink) => Ok(SinkStagePorts { inlet: sink.inlet, input_type: sink.input_type }),
      | _ => Err(StreamError::InvalidConnection),
    }
  }

  fn tick_sink_stage(&mut self, sink_position: usize, sink_index: usize) -> Result<bool, StreamError> {
    let on_tick_result = {
      let StageDefinition::Sink(sink) = &mut self.stages[sink_index] else {
        return Err(StreamError::InvalidConnection);
      };
      sink.logic.on_tick(&mut self.demand)
    };
    match on_tick_result {
      | Ok(sink_tick_progressed) => Ok(sink_tick_progressed),
      | Err(StreamError::StreamDetached) => {
        self.detach_sink_position(sink_position)?;
        Ok(true)
      },
      | Err(error) => self.apply_sink_failure(sink_position, sink_index, error),
    }
  }

  fn apply_sink_failure(
    &mut self,
    sink_position: usize,
    sink_index: usize,
    error: StreamError,
  ) -> Result<bool, StreamError> {
    match self.handle_sink_failure(sink_index, error)? {
      | FailureDisposition::Continue => Ok(true),
      | FailureDisposition::Complete => {
        self.complete_sink_position(sink_position)?;
        Ok(true)
      },
      | FailureDisposition::Fail(error) => Err(error),
    }
  }

  fn sink_can_accept_input(&self, sink_index: usize) -> Result<bool, StreamError> {
    let StageDefinition::Sink(sink) = &self.stages[sink_index] else {
      return Err(StreamError::InvalidConnection);
    };
    Ok(sink.logic.can_accept_input())
  }

  fn finish_sink_if_input_exhausted(
    &mut self,
    sink_position: usize,
    sink_index: usize,
    progressed: bool,
  ) -> Result<bool, StreamError> {
    if !self.stage_input_exhausted(sink_index) {
      return Ok(progressed);
    }

    let upstream_progressed = self.notify_sink_upstream_finish(sink_position)?;
    if self.sink_has_pending_work(sink_index)? {
      return Ok(progressed || upstream_progressed);
    }
    self.complete_sink_position(sink_position)?;
    Ok(true)
  }

  fn push_sink_input(
    &mut self,
    sink_position: usize,
    sink_index: usize,
    value: DynValue,
  ) -> Result<SinkPushOutcome, StreamError> {
    let decision_result = {
      let StageDefinition::Sink(sink) = &mut self.stages[sink_index] else {
        return Err(StreamError::InvalidConnection);
      };
      sink.logic.on_push(value, &mut self.demand)
    };
    match decision_result {
      | Ok(decision) => Ok(SinkPushOutcome::Decision(decision)),
      | Err(StreamError::StreamDetached) => {
        self.detach_sink_position(sink_position)?;
        Ok(SinkPushOutcome::Progressed)
      },
      | Err(error) => self.apply_sink_failure(sink_position, sink_index, error).map(|_| SinkPushOutcome::Progressed),
    }
  }

  fn apply_sink_decision(
    &mut self,
    sink_position: usize,
    sink_index: usize,
    decision: SinkDecision,
  ) -> Result<bool, StreamError> {
    match decision {
      | SinkDecision::Continue => Ok(true),
      | SinkDecision::Complete => {
        let (should_restart, complete_on_exhaustion) = self.schedule_sink_restart_or_complete(sink_index)?;
        if should_restart {
          return Ok(true);
        }
        if !complete_on_exhaustion {
          return Err(StreamError::Failed);
        }
        self.complete_sink_position(sink_position)?;
        Ok(true)
      },
    }
  }

  fn schedule_sink_restart_or_complete(&mut self, sink_index: usize) -> Result<(bool, bool), StreamError> {
    let StageDefinition::Sink(sink) = &mut self.stages[sink_index] else {
      return Err(StreamError::InvalidConnection);
    };
    Ok(
      if let Some(restart) = &mut sink.restart {
        (restart.schedule(self.tick_count), restart.complete_on_max_restarts())
      } else {
        (false, true)
      },
    )
  }

  fn finish_sinks(&mut self) -> Result<bool, StreamError> {
    let mut progressed = false;
    for sink_position in 0..self.sink_indices.len() {
      if self.sink_done[sink_position] {
        continue;
      }
      let sink_index = self.sink_indices[sink_position];
      self.notify_sink_upstream_finish(sink_position)?;
      if self.sink_has_pending_work(sink_index)? {
        continue;
      }
      self.complete_sink_position(sink_position)?;
      progressed = true;
    }
    Ok(progressed)
  }

  fn has_buffered_outgoing(&self, from: PortId) -> bool {
    self.connections.has_buffered_outgoing(from)
  }

  fn poll_from_incoming_edges(
    &mut self,
    to: PortId,
    preferred_slot: Option<usize>,
  ) -> Result<Option<(usize, DynValue)>, StreamError> {
    self.connections.poll_incoming_with_preferred(to, preferred_slot)
  }

  fn offer_to_next_outgoing_edge(&mut self, from: PortId, value: DynValue) -> Result<(), StreamError> {
    self.connections.offer_next(from, value)
  }

  fn offer_to_outgoing_edge(&mut self, edge_index: usize, value: DynValue) -> Result<(), StreamError> {
    self.connections.offer_at(edge_index, value)
  }

  fn outgoing_edge_indices(&self, from: PortId) -> Result<Vec<usize>, StreamError> {
    self.connections.outgoing_edge_indices(from)
  }

  fn all_edge_buffers_empty(&self) -> bool {
    self.connections.all_buffers_empty()
  }

  fn sink_has_pending_work(&self, sink_index: usize) -> Result<bool, StreamError> {
    let StageDefinition::Sink(sink) = &self.stages[sink_index] else {
      return Err(StreamError::InvalidConnection);
    };
    Ok(sink.logic.has_pending_work())
  }

  fn all_sources_done(&self) -> bool {
    self.source_done.iter().all(|done| *done)
  }

  fn set_all_sources_done(&mut self) -> Result<(), StreamError> {
    if self.all_sources_done() {
      return Ok(());
    }
    for source_position in 0..self.source_indices.len() {
      self.source_done[source_position] = true;
      let source_index = self.source_indices[source_position];
      self.close_outgoing_edges_for_stage(source_index);
    }
    self.notify_source_done_to_flows()
  }

  fn complete_source(&mut self, source_position: usize) -> Result<(), StreamError> {
    if self.source_done[source_position] {
      return Ok(());
    }
    self.source_done[source_position] = true;
    let source_index = self.source_indices[source_position];
    self.close_outgoing_edges_for_stage(source_index);
    self.notify_source_done_to_flows()
  }

  fn notify_sink_upstream_finish(&mut self, sink_position: usize) -> Result<bool, StreamError> {
    if self.sink_upstream_notified[sink_position] {
      return Ok(false);
    }
    self.sink_upstream_notified[sink_position] = true;
    let sink_index = self.sink_indices[sink_position];
    let StageDefinition::Sink(sink) = &mut self.stages[sink_index] else {
      return Err(StreamError::InvalidConnection);
    };
    sink.logic.on_upstream_finish()
  }

  fn complete_sink_position(&mut self, sink_position: usize) -> Result<(), StreamError> {
    if self.sink_done[sink_position] {
      return Ok(());
    }
    let sink_index = self.sink_indices[sink_position];
    let on_complete_result = {
      let StageDefinition::Sink(sink) = &mut self.stages[sink_index] else {
        return Err(StreamError::InvalidConnection);
      };
      sink.logic.on_complete()
    };
    self.sink_done[sink_position] = true;
    on_complete_result?;
    let incoming_edges = self.incoming_edge_indices_for_stage(sink_index);
    self.close_and_clear_incoming_edges_for_stage(sink_index)?;
    self.cancel_upstream_edges(incoming_edges)?;
    if self.all_sinks_done() && !self.has_flow_requesting_upstream_drain() {
      self.state = StreamState::Completed;
    }
    Ok(())
  }

  fn detach_sink_position(&mut self, sink_position: usize) -> Result<(), StreamError> {
    if self.sink_done[sink_position] {
      return Ok(());
    }
    self.sink_upstream_notified[sink_position] = true;
    let sink_index = self.sink_indices[sink_position];
    let incoming_edges = self.incoming_edge_indices_for_stage(sink_index);
    self.close_and_clear_incoming_edges_for_stage(sink_index)?;
    self.cancel_upstream_edges(incoming_edges)?;
    self.sink_done[sink_position] = true;
    if self.all_sinks_done() && !self.has_flow_requesting_upstream_drain() {
      // Snapshot the cancellation state BEFORE we forcibly cancel any source.
      // Otherwise the `cancel_source_if_needed` call below would always make
      // `source_canceled` non-empty, leaving the Completed branch unreachable.
      let had_live_sources = !self.all_sources_done();
      let any_canceled_before = self.source_canceled.iter().any(|canceled| *canceled);
      if had_live_sources {
        self.cancel_source_if_needed()?;
        self.set_all_sources_done()?;
      }
      // Cancelled when either the upstream propagation already cancelled a
      // source, or this method had to cancel a still-live source itself.
      self.state =
        if had_live_sources || any_canceled_before { StreamState::Cancelled } else { StreamState::Completed };
    }
    Ok(())
  }

  fn fail(&mut self, error: &StreamError) {
    if self.state.is_terminal() {
      return;
    }
    self.state = StreamState::Failed;
    for sink_position in 0..self.sink_indices.len() {
      let sink_index = self.sink_indices[sink_position];
      if let StageDefinition::Sink(sink) = &mut self.stages[sink_index] {
        sink.logic.on_error(error.clone());
      }
    }
  }

  fn flow_has_pending_output(&self, stage_index: usize) -> bool {
    let StageDefinition::Flow(flow) = &self.stages[stage_index] else {
      return false;
    };
    flow.logic.has_pending_output()
  }

  fn all_sinks_done(&self) -> bool {
    self.sink_done.iter().all(|done| *done)
  }

  fn flow_slot(&self, stage_index: usize) -> usize {
    match self.flow_slots[stage_index] {
      | Some(flow_slot) => flow_slot,
      | None => panic!("flow slot must exist for flow stage"),
    }
  }

  fn flow_source_done_at(&self, stage_index: usize) -> bool {
    self.flow_source_done[self.flow_slot(stage_index)]
  }

  fn set_flow_source_done_at(&mut self, stage_index: usize, done: bool) {
    let flow_slot = self.flow_slot(stage_index);
    self.flow_source_done[flow_slot] = done;
  }

  fn flow_done_at(&self, stage_index: usize) -> bool {
    self.flow_done[self.flow_slot(stage_index)]
  }

  fn has_flow_requesting_upstream_drain(&self) -> bool {
    self.flow_order.iter().copied().any(|stage_index| self.flow_requests_upstream_drain(stage_index))
  }

  fn flow_requests_upstream_drain(&self, stage_index: usize) -> bool {
    if self.flow_done_at(stage_index) {
      return false;
    }
    let StageDefinition::Flow(flow) = &self.stages[stage_index] else {
      return false;
    };
    flow.logic.wants_upstream_drain()
  }

  fn source_positions_for_flows_requesting_drain(&self) -> Vec<usize> {
    let mut source_positions = Vec::new();
    let mut visited_stages = Vec::new();
    for stage_index in self.flow_order.iter().copied() {
      if !self.flow_requests_upstream_drain(stage_index) {
        continue;
      }
      self.collect_upstream_source_positions(stage_index, &mut visited_stages, &mut source_positions);
    }
    source_positions
  }

  fn collect_upstream_source_positions(
    &self,
    stage_index: usize,
    visited_stages: &mut Vec<usize>,
    source_positions: &mut Vec<usize>,
  ) {
    if visited_stages.contains(&stage_index) {
      return;
    }
    visited_stages.push(stage_index);

    for edge_index in self.incoming_edge_indices_for_stage(stage_index) {
      let upstream_port = self.connections.edge_from(edge_index);
      let Some(upstream_stage_index) = self.stage_index_for_outlet(upstream_port) else {
        continue;
      };

      match &self.stages[upstream_stage_index] {
        | StageDefinition::Source(_) => {
          let Some(source_position) = self.source_indices.iter().position(|index| *index == upstream_stage_index)
          else {
            continue;
          };
          if !source_positions.contains(&source_position) {
            source_positions.push(source_position);
          }
        },
        | StageDefinition::Flow(_) => {
          self.collect_upstream_source_positions(upstream_stage_index, visited_stages, source_positions);
        },
        | StageDefinition::Sink(_) => {},
      }
    }
  }

  fn set_flow_done_at(&mut self, stage_index: usize, done: bool) {
    let flow_slot = self.flow_slot(stage_index);
    self.flow_done[flow_slot] = done;
  }

  fn mark_flow_source_done(&mut self, stage_index: usize) -> Result<bool, StreamError> {
    if self.flow_source_done_at(stage_index) || !self.stage_input_exhausted(stage_index) {
      return Ok(false);
    }
    let StageDefinition::Flow(flow) = &mut self.stages[stage_index] else {
      return Ok(false);
    };
    flow.logic.on_source_done()?;
    self.set_flow_source_done_at(stage_index, true);
    Ok(true)
  }

  fn maybe_finish_flow_stage(&mut self, stage_index: usize) -> bool {
    if self.flow_done_at(stage_index) || !self.flow_source_done_at(stage_index) {
      return false;
    }
    let StageDefinition::Flow(flow) = &self.stages[stage_index] else {
      return false;
    };
    let has_pending_output = flow.logic.has_pending_output();
    let flow_outlet = flow.outlet;
    if has_pending_output || self.has_buffered_outgoing(flow_outlet) {
      return false;
    }
    self.close_outgoing_edges_for_stage(stage_index);
    self.set_flow_done_at(stage_index, true);
    true
  }

  fn shutdown_flow_stage(&mut self, stage_index: usize) -> Result<(), StreamError> {
    let incoming_edges = self.incoming_edge_indices_for_stage(stage_index);
    self.close_and_clear_incoming_edges_for_stage(stage_index)?;
    self.cancel_upstream_edges(incoming_edges)?;
    self.mark_flow_source_done(stage_index)?;
    Ok(())
  }

  fn cancel_upstream_stage(&mut self, stage_index: usize) -> Result<(), StreamError> {
    let incoming_edges = self.incoming_edge_indices_for_stage(stage_index);
    self.cancel_upstream_edges(incoming_edges)
  }

  fn cancel_upstream_edges(&mut self, incoming_edges: Vec<usize>) -> Result<(), StreamError> {
    for edge_index in incoming_edges {
      let upstream_port = self.connections.edge_from(edge_index);
      if let Some(upstream_stage_index) = self.stage_index_for_outlet(upstream_port)
        && self.all_outgoing_edges_closed(upstream_stage_index)
      {
        if matches!(self.stages[upstream_stage_index], StageDefinition::Source(_)) {
          self.cancel_source_stage(upstream_stage_index)?;
          continue;
        }
        if matches!(self.stages[upstream_stage_index], StageDefinition::Flow(_)) {
          if self.flow_done_at(upstream_stage_index) {
            continue;
          }
          let cancel_action = if !self.flow_source_done_at(upstream_stage_index) {
            let StageDefinition::Flow(flow) = &mut self.stages[upstream_stage_index] else {
              return Err(StreamError::InvalidConnection);
            };
            flow.logic.on_downstream_cancel()?
          } else {
            DownstreamCancelAction::Propagate
          };
          if matches!(cancel_action, DownstreamCancelAction::Drain) {
            continue;
          }
          self.set_flow_source_done_at(upstream_stage_index, true);
          self.set_flow_done_at(upstream_stage_index, true);
          self.close_and_clear_incoming_edges_for_stage(upstream_stage_index)?;
          self.cancel_upstream_stage(upstream_stage_index)?;
        }
      }
    }
    Ok(())
  }

  fn cancel_source_stage(&mut self, stage_index: usize) -> Result<(), StreamError> {
    let Some(source_position) = self.source_indices.iter().position(|index| *index == stage_index) else {
      return Err(StreamError::InvalidConnection);
    };
    if self.source_done[source_position] {
      return Ok(());
    }
    let StageDefinition::Source(source) = &mut self.stages[stage_index] else {
      return Err(StreamError::InvalidConnection);
    };
    source.logic.on_cancel()?;
    self.source_done[source_position] = true;
    self.source_canceled[source_position] = true;
    Ok(())
  }

  fn stage_input_exhausted(&self, stage_index: usize) -> bool {
    let incoming_edges = self.incoming_edge_indices_for_stage(stage_index);
    !incoming_edges.is_empty()
      && incoming_edges.iter().all(|edge_index| self.connections.edge_closed_and_empty(*edge_index))
  }

  fn incoming_edge_indices_for_stage(&self, stage_index: usize) -> Vec<usize> {
    let Some(inlet) = self.stages[stage_index].inlet() else {
      return Vec::new();
    };
    self.connections.incoming_edge_indices(inlet)
  }

  fn stage_index_for_outlet(&self, outlet: PortId) -> Option<usize> {
    self
      .stages
      .iter()
      .enumerate()
      .find_map(|(index, stage)| stage.outlet().filter(|stage_outlet| *stage_outlet == outlet).map(|_| index))
  }

  fn stage_index_for_inlet(&self, inlet: PortId) -> Option<usize> {
    self
      .stages
      .iter()
      .enumerate()
      .find_map(|(index, stage)| stage.inlet().filter(|stage_inlet| *stage_inlet == inlet).map(|_| index))
  }

  fn all_outgoing_edges_closed(&self, stage_index: usize) -> bool {
    let Some(outlet) = self.stages[stage_index].outlet() else {
      return true;
    };
    self.connections.all_outgoing_closed(outlet)
  }

  fn close_outgoing_edges_for_stage(&mut self, stage_index: usize) {
    let Some(outlet) = self.stages[stage_index].outlet() else {
      return;
    };
    self.connections.close_outgoing(outlet);
  }

  fn close_and_clear_incoming_edges_for_stage(&mut self, stage_index: usize) -> Result<(), StreamError> {
    let Some(inlet) = self.stages[stage_index].inlet() else {
      return Ok(());
    };
    self.connections.close_and_clear_incoming(inlet)
  }
}
