use alloc::{vec, vec::Vec};

#[cfg(test)]
mod tests;

use super::{
  DemandTracker, DriveOutcome, DynValue, MatCombine, SinkDecision, StageDefinition, StageKind, StreamBuffer,
  StreamBufferConfig, StreamError, StreamPlan, StreamState, SupervisionStrategy, shape::PortId,
};

/// Executes a stream graph using a port-driven runtime.
pub struct GraphInterpreter {
  stages:          Vec<StageDefinition>,
  edges:           Vec<EdgeRuntime>,
  dispatch:        Vec<OutletDispatchState>,
  flow_order:      Vec<usize>,
  source_indices:  Vec<usize>,
  sink_indices:    Vec<usize>,
  demand:          DemandTracker,
  state:           StreamState,
  source_done:     Vec<bool>,
  source_canceled: Vec<bool>,
  sink_done:       Vec<bool>,
  on_start_done:   bool,
  tick_count:      u64,
}

impl GraphInterpreter {
  /// Creates a new interpreter from the provided plan.
  ///
  /// # Panics
  ///
  /// Panics when the provided plan is structurally invalid.
  #[must_use]
  pub(in crate::core) fn new(plan: StreamPlan, buffer_config: StreamBufferConfig) -> Self {
    let compiled = Self::compile_plan(plan, buffer_config);
    let source_indices_len = compiled.source_indices.len();
    let sink_indices_len = compiled.sink_indices.len();
    Self {
      stages:          compiled.stages,
      edges:           compiled.edges,
      dispatch:        compiled.dispatch,
      flow_order:      compiled.flow_order,
      source_indices:  compiled.source_indices,
      sink_indices:    compiled.sink_indices,
      demand:          DemandTracker::new(),
      state:           StreamState::Idle,
      source_done:     vec![false; source_indices_len],
      source_canceled: vec![false; source_indices_len],
      sink_done:       vec![false; sink_indices_len],
      on_start_done:   false,
      tick_count:      0,
    }
  }

  /// Starts the interpreter.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when the sink cannot start.
  pub fn start(&mut self) -> Result<(), StreamError> {
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
  pub const fn state(&self) -> StreamState {
    self.state
  }

  /// Cancels the stream.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when cancellation cannot be processed.
  pub fn cancel(&mut self) -> Result<(), StreamError> {
    if self.state.is_terminal() {
      return Ok(());
    }
    self.cancel_source_if_needed()?;
    self.set_all_sources_done()?;
    self.state = StreamState::Cancelled;
    Ok(())
  }

  pub(in crate::core) fn request_shutdown(&mut self) -> Result<(), StreamError> {
    if self.state.is_terminal() || self.all_sources_done() {
      return Ok(());
    }
    self.cancel_source_if_needed()?;
    self.set_all_sources_done()?;
    Ok(())
  }

  pub(in crate::core) fn abort(&mut self, error: &StreamError) {
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
  pub fn drive(&mut self) -> DriveOutcome {
    if self.state != StreamState::Running {
      return DriveOutcome::Idle;
    }

    self.tick_count = self.tick_count.saturating_add(1);

    if let Err(error) = self.tick_restart_windows() {
      self.fail(&error);
      return DriveOutcome::Progressed;
    }

    let mut progressed = false;

    match self.tick_flow_stages() {
      | Ok(true) => progressed = true,
      | Ok(false) => {},
      | Err(error) => {
        self.fail(&error);
        return DriveOutcome::Progressed;
      },
    }

    if !self.on_start_done {
      match self.start_sinks() {
        | Ok(()) => {
          self.on_start_done = true;
          progressed = true;
        },
        | Err(error) => {
          self.fail(&error);
          return DriveOutcome::Progressed;
        },
      }
    }

    if self.state != StreamState::Running {
      return DriveOutcome::Progressed;
    }

    if self.demand.has_demand() {
      match self.pull_sources_if_needed() {
        | Ok(did_pull) => {
          if did_pull {
            progressed = true;
          }
        },
        | Err(error) => {
          self.fail(&error);
          return DriveOutcome::Progressed;
        },
      }

      loop {
        match self.drive_flow_stages_once() {
          | Ok(true) => progressed = true,
          | Ok(false) => break,
          | Err(error) => {
            self.fail(&error);
            return DriveOutcome::Progressed;
          },
        }
      }

      match self.drive_sinks_once() {
        | Ok(true) => progressed = true,
        | Ok(false) => {},
        | Err(error) => {
          self.fail(&error);
          return DriveOutcome::Progressed;
        },
      }
    }

    if self.all_sources_done()
      && self.state == StreamState::Running
      && !self.source_restart_waiting()
      && !self.sink_restart_waiting()
      && !self.flow_order.iter().any(|stage_index| self.flow_restart_waiting(*stage_index))
      && !self.flow_order.iter().any(|stage_index| self.flow_has_pending_output(*stage_index))
    {
      loop {
        match self.drive_flow_stages_once() {
          | Ok(true) => progressed = true,
          | Ok(false) => break,
          | Err(error) => {
            self.fail(&error);
            return DriveOutcome::Progressed;
          },
        }
      }

      if self.all_edge_buffers_empty() {
        match self.finish_sinks() {
          | Ok(()) => {
            self.state = StreamState::Completed;
            progressed = true;
          },
          | Err(error) => {
            self.fail(&error);
            return DriveOutcome::Progressed;
          },
        }
      }
    }

    if progressed { DriveOutcome::Progressed } else { DriveOutcome::Idle }
  }

  fn tick_flow_stages(&mut self) -> Result<bool, StreamError> {
    let mut progressed = false;

    for flow_index in 0..self.flow_order.len() {
      let stage_index = self.flow_order[flow_index];
      if self.flow_restart_waiting(stage_index) {
        continue;
      }

      let on_tick_result = {
        let StageDefinition::Flow(flow) = &mut self.stages[stage_index] else {
          continue;
        };
        flow.logic.on_tick(self.tick_count)
      };
      match on_tick_result {
        | Ok(()) => {},
        | Err(error) => match self.handle_flow_failure(stage_index, error)? {
          | FailureDisposition::Continue => {
            progressed = true;
            continue;
          },
          | FailureDisposition::Complete => {
            self.set_all_sources_done()?;
            self.notify_source_done_to_flows()?;
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
        self.request_shutdown()?;
        progressed = true;
      }
    }

    Ok(progressed)
  }

  fn compile_plan(plan: StreamPlan, buffer_config: StreamBufferConfig) -> CompiledPlan {
    let StreamPlan { stages, edges, source_indices, sink_indices, flow_order, .. } = plan;

    let mut runtime_edges = Vec::new();
    for edge in edges {
      runtime_edges.push(EdgeRuntime {
        from:   edge.from_port,
        to:     edge.to_port,
        _mat:   edge.mat,
        buffer: StreamBuffer::new(buffer_config),
      });
    }

    let dispatch = Self::create_dispatch_states(&stages);

    CompiledPlan { stages, edges: runtime_edges, dispatch, flow_order, source_indices, sink_indices }
  }

  fn create_dispatch_states(stages: &[StageDefinition]) -> Vec<OutletDispatchState> {
    let mut dispatch = Vec::new();
    for stage in stages {
      match stage {
        | StageDefinition::Source(source) => dispatch.push(OutletDispatchState::new(source.outlet)),
        | StageDefinition::Flow(flow) => dispatch.push(OutletDispatchState::new(flow.outlet)),
        | StageDefinition::Sink(_) => {},
      }
    }
    dispatch
  }

  fn start_sinks(&mut self) -> Result<(), StreamError> {
    for sink_index in &self.sink_indices {
      let StageDefinition::Sink(sink) = &mut self.stages[*sink_index] else {
        return Err(StreamError::InvalidConnection);
      };
      sink.logic.on_start(&mut self.demand)?;
    }
    Ok(())
  }

  fn notify_source_done_to_flows(&mut self) -> Result<(), StreamError> {
    for stage in &mut self.stages {
      if let StageDefinition::Flow(flow) = stage {
        flow.logic.on_source_done()?;
      }
    }
    Ok(())
  }

  fn cancel_source_if_needed(&mut self) -> Result<(), StreamError> {
    for source_position in 0..self.source_indices.len() {
      if self.source_canceled[source_position] {
        continue;
      }
      let source_index = self.source_indices[source_position];
      let StageDefinition::Source(source) = &mut self.stages[source_index] else {
        return Err(StreamError::InvalidConnection);
      };
      source.logic.on_cancel()?;
      self.source_canceled[source_position] = true;
    }
    Ok(())
  }

  fn pull_sources_if_needed(&mut self) -> Result<bool, StreamError> {
    let mut progressed = false;

    for source_position in 0..self.source_indices.len() {
      if self.source_done[source_position] {
        continue;
      }
      if self.source_restart_waiting_at(source_position) {
        continue;
      }

      let source_index = self.source_indices[source_position];
      let (source_outlet, source_output_type) = match &self.stages[source_index] {
        | StageDefinition::Source(source) => (source.outlet, source.output_type),
        | _ => return Err(StreamError::InvalidConnection),
      };

      if self.has_buffered_outgoing(source_outlet) {
        continue;
      }

      let pulled_result = {
        let StageDefinition::Source(source) = &mut self.stages[source_index] else {
          return Err(StreamError::InvalidConnection);
        };
        source.logic.pull()
      };

      let pulled = match pulled_result {
        | Ok(pulled) => pulled,
        | Err(StreamError::WouldBlock) => continue,
        | Err(error) => match self.handle_source_failure(source_position, error)? {
          | FailureDisposition::Continue => {
            progressed = true;
            continue;
          },
          | FailureDisposition::Complete => {
            self.complete_source(source_position)?;
            progressed = true;
            continue;
          },
          | FailureDisposition::Fail(error) => return Err(error),
        },
      };

      match pulled {
        | Some(value) => {
          if value.as_ref().type_id() != source_output_type {
            return Err(StreamError::TypeMismatch);
          }
          self.offer_to_next_outgoing_edge(source_outlet, value)?;
          progressed = true;
        },
        | None => {
          let (should_restart, complete_on_exhaustion) = {
            let StageDefinition::Source(source) = &mut self.stages[source_index] else {
              return Err(StreamError::InvalidConnection);
            };
            if let Some(restart) = &mut source.restart {
              (restart.schedule(self.tick_count), restart.complete_on_max_restarts())
            } else {
              (false, true)
            }
          };
          if should_restart {
            progressed = true;
            continue;
          }
          if !complete_on_exhaustion {
            return Err(StreamError::Failed);
          }
          self.complete_source(source_position)?;
          progressed = true;
        },
      }
    }

    Ok(progressed)
  }

  fn drive_flow_stages_once(&mut self) -> Result<bool, StreamError> {
    let mut progressed = false;

    for flow_index in 0..self.flow_order.len() {
      let stage_index = self.flow_order[flow_index];
      if self.flow_restart_waiting(stage_index) {
        continue;
      }
      let (flow_inlet, flow_outlet, flow_input_type, flow_output_type) = match &self.stages[stage_index] {
        | StageDefinition::Flow(flow) => (flow.inlet, flow.outlet, flow.input_type, flow.output_type),
        | _ => continue,
      };
      if self.has_buffered_outgoing(flow_outlet) {
        continue;
      }

      let mut consumed_input = false;
      let mut outputs = Vec::new();

      let can_accept_input = match &self.stages[stage_index] {
        | StageDefinition::Flow(flow) => flow.logic.can_accept_input(),
        | _ => false,
      };

      if can_accept_input && let Some((edge_index, input)) = self.poll_from_incoming_edges(flow_inlet)? {
        consumed_input = true;
        if input.as_ref().type_id() != flow_input_type {
          return Err(StreamError::TypeMismatch);
        }

        let apply_result = {
          let StageDefinition::Flow(flow) = &mut self.stages[stage_index] else {
            return Err(StreamError::InvalidConnection);
          };
          flow.logic.apply_with_edge(edge_index, input)
        };
        outputs = match apply_result {
          | Ok(outputs) => outputs,
          | Err(error) => match self.handle_flow_failure(stage_index, error)? {
            | FailureDisposition::Continue => {
              progressed = true;
              continue;
            },
            | FailureDisposition::Complete => {
              self.set_all_sources_done()?;
              self.notify_source_done_to_flows()?;
              progressed = true;
              continue;
            },
            | FailureDisposition::Fail(error) => return Err(error),
          },
        };
      }

      if outputs.is_empty() {
        let drain_result = {
          let StageDefinition::Flow(flow) = &mut self.stages[stage_index] else {
            return Err(StreamError::InvalidConnection);
          };
          flow.logic.drain_pending()
        };
        outputs = match drain_result {
          | Ok(outputs) => outputs,
          | Err(error) => match self.handle_flow_failure(stage_index, error)? {
            | FailureDisposition::Continue => {
              progressed = true;
              continue;
            },
            | FailureDisposition::Complete => {
              self.set_all_sources_done()?;
              self.notify_source_done_to_flows()?;
              progressed = true;
              continue;
            },
            | FailureDisposition::Fail(error) => return Err(error),
          },
        };
      }

      if consumed_input {
        progressed = true;
      }

      let shutdown_requested = {
        let StageDefinition::Flow(flow) = &mut self.stages[stage_index] else {
          return Err(StreamError::InvalidConnection);
        };
        flow.logic.take_shutdown_request()
      };
      if shutdown_requested {
        self.request_shutdown()?;
        progressed = true;
      }

      if outputs.is_empty() {
        continue;
      }

      for output in outputs {
        if output.as_ref().type_id() != flow_output_type {
          return Err(StreamError::TypeMismatch);
        }
        self.offer_to_next_outgoing_edge(flow_outlet, output)?;
      }
      progressed = true;
    }

    Ok(progressed)
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
    if !self.demand.has_demand() {
      return Ok(false);
    }
    let sink_index = self.sink_indices[sink_position];
    let (sink_inlet, sink_input_type) = match &self.stages[sink_index] {
      | StageDefinition::Sink(sink) => (sink.inlet, sink.input_type),
      | _ => return Err(StreamError::InvalidConnection),
    };
    if self.sink_restart_waiting_at(sink_index) {
      return Ok(false);
    }
    let sink_can_accept = {
      let StageDefinition::Sink(sink) = &self.stages[sink_index] else {
        return Err(StreamError::InvalidConnection);
      };
      sink.logic.can_accept_input()
    };
    if !sink_can_accept {
      return Ok(false);
    }

    let Some((_, value)) = self.poll_from_incoming_edges(sink_inlet)? else {
      return Ok(false);
    };
    if value.as_ref().type_id() != sink_input_type {
      return Err(StreamError::TypeMismatch);
    }
    self.demand.consume(1)?;

    let decision_result = {
      let StageDefinition::Sink(sink) = &mut self.stages[sink_index] else {
        return Err(StreamError::InvalidConnection);
      };
      sink.logic.on_push(value, &mut self.demand)
    };
    let decision = match decision_result {
      | Ok(decision) => decision,
      | Err(error) => match self.handle_sink_failure(sink_index, error)? {
        | FailureDisposition::Continue => return Ok(true),
        | FailureDisposition::Complete => {
          self.sink_done[sink_position] = true;
          if self.all_sinks_done() {
            self.finish_sinks()?;
            self.state = StreamState::Completed;
          }
          return Ok(true);
        },
        | FailureDisposition::Fail(error) => return Err(error),
      },
    };
    match decision {
      | SinkDecision::Continue => Ok(true),
      | SinkDecision::Complete => {
        let (should_restart, complete_on_exhaustion) = {
          let StageDefinition::Sink(sink) = &mut self.stages[sink_index] else {
            return Err(StreamError::InvalidConnection);
          };
          if let Some(restart) = &mut sink.restart {
            (restart.schedule(self.tick_count), restart.complete_on_max_restarts())
          } else {
            (false, true)
          }
        };
        if should_restart {
          return Ok(true);
        }
        if !complete_on_exhaustion {
          return Err(StreamError::Failed);
        }
        self.sink_done[sink_position] = true;
        if self.all_sinks_done() {
          self.finish_sinks()?;
          self.state = StreamState::Completed;
        }
        Ok(true)
      },
    }
  }

  fn finish_sinks(&mut self) -> Result<(), StreamError> {
    for sink_position in 0..self.sink_indices.len() {
      if self.sink_done[sink_position] {
        continue;
      }
      let sink_index = self.sink_indices[sink_position];
      let StageDefinition::Sink(sink) = &mut self.stages[sink_index] else {
        return Err(StreamError::InvalidConnection);
      };
      sink.logic.on_complete()?;
      self.sink_done[sink_position] = true;
    }
    Ok(())
  }

  fn has_buffered_outgoing(&self, from: PortId) -> bool {
    self.edges.iter().any(|edge| edge.from == from && !edge.buffer.is_empty())
  }

  fn poll_from_incoming_edges(&mut self, to: PortId) -> Result<Option<(usize, DynValue)>, StreamError> {
    for (index, edge) in self.edges.iter_mut().enumerate() {
      if edge.to != to || edge.buffer.is_empty() {
        continue;
      }
      let value = edge.buffer.poll()?;
      return Ok(Some((index, value)));
    }
    Ok(None)
  }

  fn offer_to_next_outgoing_edge(&mut self, from: PortId, value: DynValue) -> Result<(), StreamError> {
    let target = self.next_outgoing_edge_index(from)?;

    if self.edges[target].buffer.offer(value).is_err() {
      return Err(StreamError::BufferOverflow);
    }
    Ok(())
  }

  fn next_outgoing_edge_index(&mut self, from: PortId) -> Result<usize, StreamError> {
    let mut outgoing_edges = Vec::new();
    for (index, edge) in self.edges.iter().enumerate() {
      if edge.from == from {
        outgoing_edges.push(index);
      }
    }

    if outgoing_edges.is_empty() {
      return Err(StreamError::InvalidConnection);
    }

    let Some(state_index) = self.dispatch.iter().position(|state| state.outlet == from) else {
      return Err(StreamError::InvalidConnection);
    };
    let next = self.dispatch[state_index].next_edge % outgoing_edges.len();
    self.dispatch[state_index].next_edge = (next + 1) % outgoing_edges.len();
    Ok(outgoing_edges[next])
  }

  fn all_edge_buffers_empty(&self) -> bool {
    self.edges.iter().all(|edge| edge.buffer.is_empty())
  }

  fn all_sources_done(&self) -> bool {
    self.source_done.iter().all(|done| *done)
  }

  fn set_all_sources_done(&mut self) -> Result<(), StreamError> {
    if self.all_sources_done() {
      return Ok(());
    }
    self.source_done.iter_mut().for_each(|done| *done = true);
    self.notify_source_done_to_flows()
  }

  fn complete_source(&mut self, source_position: usize) -> Result<(), StreamError> {
    if self.source_done[source_position] {
      return Ok(());
    }
    self.source_done[source_position] = true;
    self.notify_source_done_to_flows()
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

  fn tick_restart_windows(&mut self) -> Result<(), StreamError> {
    for stage in &mut self.stages {
      match stage {
        | StageDefinition::Source(source) => {
          if let Some(restart) = &mut source.restart
            && restart.tick(self.tick_count)
          {
            source.logic.on_restart()?;
          }
        },
        | StageDefinition::Flow(flow) => {
          if let Some(restart) = &mut flow.restart
            && restart.tick(self.tick_count)
          {
            flow.logic.on_restart()?;
          }
        },
        | StageDefinition::Sink(sink) => {
          if let Some(restart) = &mut sink.restart
            && restart.tick(self.tick_count)
          {
            sink.logic.on_restart()?;
            sink.logic.on_start(&mut self.demand)?;
          }
        },
      }
    }
    Ok(())
  }

  fn source_restart_waiting(&self) -> bool {
    for source_index in &self.source_indices {
      let StageDefinition::Source(source) = &self.stages[*source_index] else {
        return false;
      };
      if source.restart.map(|restart| restart.is_waiting()).unwrap_or(false) {
        return true;
      }
    }
    false
  }

  fn source_restart_waiting_at(&self, source_position: usize) -> bool {
    let source_index = self.source_indices[source_position];
    let StageDefinition::Source(source) = &self.stages[source_index] else {
      return false;
    };
    source.restart.map(|restart| restart.is_waiting()).unwrap_or(false)
  }

  fn flow_restart_waiting(&self, stage_index: usize) -> bool {
    let StageDefinition::Flow(flow) = &self.stages[stage_index] else {
      return false;
    };
    flow.restart.map(|restart| restart.is_waiting()).unwrap_or(false)
  }

  fn flow_has_pending_output(&self, stage_index: usize) -> bool {
    let StageDefinition::Flow(flow) = &self.stages[stage_index] else {
      return false;
    };
    flow.logic.has_pending_output()
  }

  fn sink_restart_waiting(&self) -> bool {
    for sink_index in &self.sink_indices {
      if self.sink_restart_waiting_at(*sink_index) {
        return true;
      }
    }
    false
  }

  fn sink_restart_waiting_at(&self, sink_index: usize) -> bool {
    let StageDefinition::Sink(sink) = &self.stages[sink_index] else {
      return false;
    };
    sink.restart.map(|restart| restart.is_waiting()).unwrap_or(false)
  }

  fn all_sinks_done(&self) -> bool {
    self.sink_done.iter().all(|done| *done)
  }

  fn handle_source_failure(
    &mut self,
    source_position: usize,
    error: StreamError,
  ) -> Result<FailureDisposition, StreamError> {
    let source_index = self.source_indices[source_position];
    let StageDefinition::Source(source) = &mut self.stages[source_index] else {
      return Ok(FailureDisposition::Fail(StreamError::InvalidConnection));
    };
    if let Some(restart) = &mut source.restart {
      if restart.schedule(self.tick_count) {
        return Ok(FailureDisposition::Continue);
      }
      return if restart.complete_on_max_restarts() {
        Ok(FailureDisposition::Complete)
      } else {
        Ok(FailureDisposition::Fail(error))
      };
    }
    match source.supervision {
      | SupervisionStrategy::Stop => Ok(FailureDisposition::Fail(error)),
      | SupervisionStrategy::Resume => Ok(FailureDisposition::Continue),
      | SupervisionStrategy::Restart => {
        source.logic.on_restart()?;
        Ok(FailureDisposition::Continue)
      },
    }
  }

  fn handle_flow_failure(&mut self, stage_index: usize, error: StreamError) -> Result<FailureDisposition, StreamError> {
    let StageDefinition::Flow(flow) = &mut self.stages[stage_index] else {
      return Ok(FailureDisposition::Fail(StreamError::InvalidConnection));
    };
    if let Some(restart) = &mut flow.restart {
      if restart.schedule(self.tick_count) {
        return Ok(FailureDisposition::Continue);
      }
      return if restart.complete_on_max_restarts() {
        Ok(FailureDisposition::Complete)
      } else {
        Ok(FailureDisposition::Fail(error))
      };
    }
    match flow.supervision {
      | SupervisionStrategy::Stop => Ok(FailureDisposition::Fail(error)),
      | SupervisionStrategy::Resume => Ok(FailureDisposition::Continue),
      | SupervisionStrategy::Restart => {
        if matches!(flow.kind, StageKind::FlowSplitWhen | StageKind::FlowSplitAfter) {
          return Ok(FailureDisposition::Continue);
        }
        flow.logic.on_restart()?;
        Ok(FailureDisposition::Continue)
      },
    }
  }

  fn handle_sink_failure(&mut self, sink_index: usize, error: StreamError) -> Result<FailureDisposition, StreamError> {
    let StageDefinition::Sink(sink) = &mut self.stages[sink_index] else {
      return Ok(FailureDisposition::Fail(StreamError::InvalidConnection));
    };
    if let Some(restart) = &mut sink.restart {
      if restart.schedule(self.tick_count) {
        self.demand.request(1)?;
        return Ok(FailureDisposition::Continue);
      }
      return if restart.complete_on_max_restarts() {
        Ok(FailureDisposition::Complete)
      } else {
        Ok(FailureDisposition::Fail(error))
      };
    }
    match sink.supervision {
      | SupervisionStrategy::Stop => Ok(FailureDisposition::Fail(error)),
      | SupervisionStrategy::Resume => {
        self.demand.request(1)?;
        Ok(FailureDisposition::Continue)
      },
      | SupervisionStrategy::Restart => {
        sink.logic.on_restart()?;
        sink.logic.on_start(&mut self.demand)?;
        Ok(FailureDisposition::Continue)
      },
    }
  }
}

struct CompiledPlan {
  stages:         Vec<StageDefinition>,
  edges:          Vec<EdgeRuntime>,
  dispatch:       Vec<OutletDispatchState>,
  flow_order:     Vec<usize>,
  source_indices: Vec<usize>,
  sink_indices:   Vec<usize>,
}

struct EdgeRuntime {
  from:   PortId,
  to:     PortId,
  _mat:   MatCombine,
  buffer: StreamBuffer<DynValue>,
}

struct OutletDispatchState {
  outlet:    PortId,
  next_edge: usize,
}

impl OutletDispatchState {
  const fn new(outlet: PortId) -> Self {
    Self { outlet, next_edge: 0 }
  }
}

enum FailureDisposition {
  Continue,
  Complete,
  Fail(StreamError),
}
