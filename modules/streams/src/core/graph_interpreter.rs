use alloc::vec::Vec;

#[cfg(test)]
mod tests;

use super::{
  DemandTracker, DriveOutcome, DynValue, MatCombine, PortId, SinkDecision, StageDefinition, StageKind, StreamBuffer,
  StreamBufferConfig, StreamError, StreamPlan, StreamState, SupervisionStrategy,
};

/// Executes a stream graph using a port-driven runtime.
pub struct GraphInterpreter {
  stages:          Vec<StageDefinition>,
  edges:           Vec<EdgeRuntime>,
  dispatch:        Vec<OutletDispatchState>,
  flow_order:      Vec<usize>,
  source_index:    usize,
  sink_index:      usize,
  demand:          DemandTracker,
  state:           StreamState,
  source_done:     bool,
  source_canceled: bool,
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
  pub(super) fn new(plan: StreamPlan, buffer_config: StreamBufferConfig) -> Self {
    let compiled = match Self::compile_plan(plan, buffer_config) {
      | Ok(compiled) => compiled,
      | Err(error) => panic!("invalid stream plan: {error}"),
    };
    Self {
      stages:          compiled.stages,
      edges:           compiled.edges,
      dispatch:        compiled.dispatch,
      flow_order:      compiled.flow_order,
      source_index:    compiled.source_index,
      sink_index:      compiled.sink_index,
      demand:          DemandTracker::new(),
      state:           StreamState::Idle,
      source_done:     false,
      source_canceled: false,
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
      self.start_sink()?;
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
    self.source_done = true;
    self.notify_source_done_to_flows()?;
    self.state = StreamState::Cancelled;
    Ok(())
  }

  pub(super) fn request_shutdown(&mut self) -> Result<(), StreamError> {
    if self.state.is_terminal() || self.source_done {
      return Ok(());
    }
    self.cancel_source_if_needed()?;
    self.source_done = true;
    self.notify_source_done_to_flows()?;
    Ok(())
  }

  pub(super) fn abort(&mut self, error: StreamError) {
    if self.state.is_terminal() {
      return;
    }
    if let Err(cancel_error) = self.cancel_source_if_needed() {
      self.fail(cancel_error);
      return;
    }
    self.source_done = true;
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
      self.fail(error);
      return DriveOutcome::Progressed;
    }

    let mut progressed = false;

    if !self.on_start_done {
      match self.start_sink() {
        | Ok(()) => {
          self.on_start_done = true;
          progressed = true;
        },
        | Err(error) => {
          self.fail(error);
          return DriveOutcome::Progressed;
        },
      }
    }

    if self.state != StreamState::Running {
      return DriveOutcome::Progressed;
    }

    if self.demand.has_demand() {
      match self.pull_source_if_needed() {
        | Ok(did_pull) => {
          if did_pull {
            progressed = true;
          }
        },
        | Err(error) => {
          self.fail(error);
          return DriveOutcome::Progressed;
        },
      }

      loop {
        match self.drive_flow_stages_once() {
          | Ok(true) => progressed = true,
          | Ok(false) => break,
          | Err(error) => {
            self.fail(error);
            return DriveOutcome::Progressed;
          },
        }
      }

      match self.drive_sink_once() {
        | Ok(true) => progressed = true,
        | Ok(false) => {},
        | Err(error) => {
          self.fail(error);
          return DriveOutcome::Progressed;
        },
      }
    }

    if self.source_done
      && self.state == StreamState::Running
      && !self.source_restart_waiting()
      && !self.sink_restart_waiting()
      && !self.flow_order.iter().any(|stage_index| self.flow_restart_waiting(*stage_index))
    {
      loop {
        match self.drive_flow_stages_once() {
          | Ok(true) => progressed = true,
          | Ok(false) => break,
          | Err(error) => {
            self.fail(error);
            return DriveOutcome::Progressed;
          },
        }
      }

      if self.all_edge_buffers_empty() {
        match self.finish_sink() {
          | Ok(()) => {
            self.state = StreamState::Completed;
            progressed = true;
          },
          | Err(error) => {
            self.fail(error);
            return DriveOutcome::Progressed;
          },
        }
      }
    }

    if progressed { DriveOutcome::Progressed } else { DriveOutcome::Idle }
  }

  fn compile_plan(plan: StreamPlan, buffer_config: StreamBufferConfig) -> Result<CompiledPlan, StreamError> {
    let StreamPlan { stages, edges, .. } = plan;
    if stages.is_empty() || edges.is_empty() {
      return Err(StreamError::InvalidConnection);
    }

    let mut source_index = None;
    let mut sink_index = None;
    for (index, stage) in stages.iter().enumerate() {
      match stage {
        | StageDefinition::Source(_) => {
          if source_index.replace(index).is_some() {
            return Err(StreamError::InvalidConnection);
          }
        },
        | StageDefinition::Sink(_) => {
          if sink_index.replace(index).is_some() {
            return Err(StreamError::InvalidConnection);
          }
        },
        | StageDefinition::Flow(_) => {},
      }
    }

    let Some(source_index) = source_index else {
      return Err(StreamError::InvalidConnection);
    };
    let Some(sink_index) = sink_index else {
      return Err(StreamError::InvalidConnection);
    };

    let mut runtime_edges = Vec::new();
    for (from, to, mat) in edges {
      if !Self::has_output_port(&stages, from) || !Self::has_input_port(&stages, to) {
        return Err(StreamError::InvalidConnection);
      }
      runtime_edges.push(EdgeRuntime { from, to, _mat: mat, buffer: StreamBuffer::new(buffer_config) });
    }

    let flow_order = Self::compute_flow_order(&stages, &runtime_edges)?;

    for stage in &stages {
      match stage {
        | StageDefinition::Source(source) => {
          if runtime_edges.iter().filter(|edge| edge.from == source.outlet).count() == 0 {
            return Err(StreamError::InvalidConnection);
          }
        },
        | StageDefinition::Flow(flow) => {
          let incoming_count = runtime_edges.iter().filter(|edge| edge.to == flow.inlet).count();
          if incoming_count == 0 {
            return Err(StreamError::InvalidConnection);
          }
          if let Some(expected_fan_in) = flow.logic.expected_fan_in()
            && incoming_count != expected_fan_in
          {
            return Err(StreamError::InvalidConnection);
          }
          let outgoing_count = runtime_edges.iter().filter(|edge| edge.from == flow.outlet).count();
          if outgoing_count == 0 {
            return Err(StreamError::InvalidConnection);
          }
          if let Some(expected_fan_out) = flow.logic.expected_fan_out()
            && outgoing_count != expected_fan_out
          {
            return Err(StreamError::InvalidConnection);
          }
        },
        | StageDefinition::Sink(sink) => {
          if runtime_edges.iter().filter(|edge| edge.to == sink.inlet).count() == 0 {
            return Err(StreamError::InvalidConnection);
          }
        },
      }
    }

    let dispatch = Self::create_dispatch_states(&stages);

    Ok(CompiledPlan { stages, edges: runtime_edges, dispatch, flow_order, source_index, sink_index })
  }

  fn has_input_port(stages: &[StageDefinition], port: PortId) -> bool {
    stages.iter().any(|stage| stage.inlet() == Some(port))
  }

  fn has_output_port(stages: &[StageDefinition], port: PortId) -> bool {
    stages.iter().any(|stage| stage.outlet() == Some(port))
  }

  fn stage_index_from_input_port(stages: &[StageDefinition], port: PortId) -> Option<usize> {
    stages.iter().position(|stage| stage.inlet() == Some(port))
  }

  fn stage_index_from_output_port(stages: &[StageDefinition], port: PortId) -> Option<usize> {
    stages.iter().position(|stage| stage.outlet() == Some(port))
  }

  fn compute_flow_order(stages: &[StageDefinition], edges: &[EdgeRuntime]) -> Result<Vec<usize>, StreamError> {
    let mut incoming = Vec::new();
    let mut outgoing: Vec<Vec<usize>> = Vec::new();
    for _ in 0..stages.len() {
      incoming.push(0_usize);
      outgoing.push(Vec::new());
    }

    for edge in edges {
      let Some(from_stage) = Self::stage_index_from_output_port(stages, edge.from) else {
        return Err(StreamError::InvalidConnection);
      };
      let Some(to_stage) = Self::stage_index_from_input_port(stages, edge.to) else {
        return Err(StreamError::InvalidConnection);
      };
      outgoing[from_stage].push(to_stage);
      incoming[to_stage] = incoming[to_stage].saturating_add(1);
    }

    let mut ready = Vec::new();
    for (stage_index, count) in incoming.iter().enumerate() {
      if *count == 0 {
        ready.push(stage_index);
      }
    }

    let mut stage_order = Vec::new();
    while let Some(stage_index) = ready.pop() {
      stage_order.push(stage_index);
      for next_index in &outgoing[stage_index] {
        incoming[*next_index] = incoming[*next_index].saturating_sub(1);
        if incoming[*next_index] == 0 {
          ready.push(*next_index);
        }
      }
    }

    if stage_order.len() != stages.len() {
      return Err(StreamError::InvalidConnection);
    }

    Ok(stage_order.into_iter().filter(|stage_index| matches!(stages[*stage_index], StageDefinition::Flow(_))).collect())
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

  fn start_sink(&mut self) -> Result<(), StreamError> {
    let StageDefinition::Sink(sink) = &mut self.stages[self.sink_index] else {
      return Err(StreamError::InvalidConnection);
    };
    sink.logic.on_start(&mut self.demand)
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
    if self.source_canceled {
      return Ok(());
    }
    let StageDefinition::Source(source) = &mut self.stages[self.source_index] else {
      return Err(StreamError::InvalidConnection);
    };
    source.logic.on_cancel()?;
    self.source_canceled = true;
    Ok(())
  }

  fn pull_source_if_needed(&mut self) -> Result<bool, StreamError> {
    if self.source_done {
      return Ok(false);
    }
    if self.source_restart_waiting() {
      return Ok(false);
    }
    let (source_outlet, source_output_type) = match &self.stages[self.source_index] {
      | StageDefinition::Source(source) => (source.outlet, source.output_type),
      | _ => return Err(StreamError::InvalidConnection),
    };

    if self.has_buffered_outgoing(source_outlet) {
      return Ok(false);
    }

    let pulled_result = {
      let StageDefinition::Source(source) = &mut self.stages[self.source_index] else {
        return Err(StreamError::InvalidConnection);
      };
      source.logic.pull()
    };

    let pulled = match pulled_result {
      | Ok(pulled) => pulled,
      | Err(StreamError::WouldBlock) => return Ok(false),
      | Err(error) => match self.handle_source_failure(error)? {
        | FailureDisposition::Continue => return Ok(true),
        | FailureDisposition::Complete => {
          self.source_done = true;
          self.notify_source_done_to_flows()?;
          return Ok(true);
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
        Ok(true)
      },
      | None => {
        let (should_restart, complete_on_exhaustion) = {
          let StageDefinition::Source(source) = &mut self.stages[self.source_index] else {
            return Err(StreamError::InvalidConnection);
          };
          if let Some(restart) = &mut source.restart {
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
        self.source_done = true;
        self.notify_source_done_to_flows()?;
        Ok(true)
      },
    }
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
              self.source_done = true;
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
              self.source_done = true;
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

  fn drive_sink_once(&mut self) -> Result<bool, StreamError> {
    if self.sink_restart_waiting() {
      return Ok(false);
    }
    if !self.demand.has_demand() {
      return Ok(false);
    }
    let sink_can_accept = match &self.stages[self.sink_index] {
      | StageDefinition::Sink(sink) => sink.logic.can_accept_input(),
      | _ => false,
    };
    if !sink_can_accept {
      return Ok(false);
    }
    let (sink_inlet, sink_input_type) = match &self.stages[self.sink_index] {
      | StageDefinition::Sink(sink) => (sink.inlet, sink.input_type),
      | _ => return Err(StreamError::InvalidConnection),
    };

    let Some((_, value)) = self.poll_from_incoming_edges(sink_inlet)? else {
      return Ok(false);
    };
    if value.as_ref().type_id() != sink_input_type {
      return Err(StreamError::TypeMismatch);
    }
    self.demand.consume(1)?;

    let decision_result = {
      let StageDefinition::Sink(sink) = &mut self.stages[self.sink_index] else {
        return Err(StreamError::InvalidConnection);
      };
      sink.logic.on_push(value, &mut self.demand)
    };
    let decision = match decision_result {
      | Ok(decision) => decision,
      | Err(error) => match self.handle_sink_failure(error)? {
        | FailureDisposition::Continue => return Ok(true),
        | FailureDisposition::Complete => {
          self.finish_sink()?;
          self.state = StreamState::Completed;
          return Ok(true);
        },
        | FailureDisposition::Fail(error) => return Err(error),
      },
    };
    match decision {
      | SinkDecision::Continue => Ok(true),
      | SinkDecision::Complete => {
        let (should_restart, complete_on_exhaustion) = {
          let StageDefinition::Sink(sink) = &mut self.stages[self.sink_index] else {
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
        self.finish_sink()?;
        self.state = StreamState::Completed;
        Ok(true)
      },
    }
  }

  fn finish_sink(&mut self) -> Result<(), StreamError> {
    let StageDefinition::Sink(sink) = &mut self.stages[self.sink_index] else {
      return Err(StreamError::InvalidConnection);
    };
    sink.logic.on_complete()
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

  fn fail(&mut self, error: StreamError) {
    if self.state.is_terminal() {
      return;
    }
    self.state = StreamState::Failed;
    if let StageDefinition::Sink(sink) = &mut self.stages[self.sink_index] {
      sink.logic.on_error(error);
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
    let StageDefinition::Source(source) = &self.stages[self.source_index] else {
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

  fn sink_restart_waiting(&self) -> bool {
    let StageDefinition::Sink(sink) = &self.stages[self.sink_index] else {
      return false;
    };
    sink.restart.map(|restart| restart.is_waiting()).unwrap_or(false)
  }

  fn handle_source_failure(&mut self, error: StreamError) -> Result<FailureDisposition, StreamError> {
    let StageDefinition::Source(source) = &mut self.stages[self.source_index] else {
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

  fn handle_sink_failure(&mut self, error: StreamError) -> Result<FailureDisposition, StreamError> {
    let StageDefinition::Sink(sink) = &mut self.stages[self.sink_index] else {
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
  stages:       Vec<StageDefinition>,
  edges:        Vec<EdgeRuntime>,
  dispatch:     Vec<OutletDispatchState>,
  flow_order:   Vec<usize>,
  source_index: usize,
  sink_index:   usize,
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
