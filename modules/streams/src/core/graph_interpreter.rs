use alloc::vec::Vec;

#[cfg(test)]
mod tests;

use super::{
  DemandTracker, DriveOutcome, DynValue, MatCombine, PortId, SinkDecision, StageDefinition, StreamBuffer,
  StreamBufferConfig, StreamError, StreamPlan, StreamState,
};

/// Executes a stream graph using a port-driven runtime.
pub struct GraphInterpreter {
  stages:        Vec<StageDefinition>,
  edges:         Vec<EdgeRuntime>,
  dispatch:      Vec<OutletDispatchState>,
  flow_order:    Vec<usize>,
  source_index:  usize,
  sink_index:    usize,
  demand:        DemandTracker,
  state:         StreamState,
  source_done:   bool,
  on_start_done: bool,
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
      stages:        compiled.stages,
      edges:         compiled.edges,
      dispatch:      compiled.dispatch,
      flow_order:    compiled.flow_order,
      source_index:  compiled.source_index,
      sink_index:    compiled.sink_index,
      demand:        DemandTracker::new(),
      state:         StreamState::Idle,
      source_done:   false,
      on_start_done: false,
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
  pub const fn cancel(&mut self) -> Result<(), StreamError> {
    if self.state.is_terminal() {
      return Ok(());
    }
    self.state = StreamState::Cancelled;
    Ok(())
  }

  /// Drives the interpreter once.
  #[must_use]
  pub fn drive(&mut self) -> DriveOutcome {
    if self.state != StreamState::Running {
      return DriveOutcome::Idle;
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

    if self.source_done && self.all_edge_buffers_empty() && self.state == StreamState::Running {
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

    if progressed { DriveOutcome::Progressed } else { DriveOutcome::Idle }
  }

  fn compile_plan(plan: StreamPlan, buffer_config: StreamBufferConfig) -> Result<CompiledPlan, StreamError> {
    let StreamPlan { stages, edges } = plan;
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

  fn pull_source_if_needed(&mut self) -> Result<bool, StreamError> {
    if self.source_done {
      return Ok(false);
    }
    let (source_outlet, source_output_type) = match &self.stages[self.source_index] {
      | StageDefinition::Source(source) => (source.outlet, source.output_type),
      | _ => return Err(StreamError::InvalidConnection),
    };

    if self.has_buffered_outgoing(source_outlet) {
      return Ok(false);
    }

    let pulled = {
      let StageDefinition::Source(source) = &mut self.stages[self.source_index] else {
        return Err(StreamError::InvalidConnection);
      };
      source.logic.pull()
    }?;

    match pulled {
      | Some(value) => {
        if value.as_ref().type_id() != source_output_type {
          return Err(StreamError::TypeMismatch);
        }
        self.offer_to_next_outgoing_edge(source_outlet, value)?;
        Ok(true)
      },
      | None => {
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
      let (flow_inlet, flow_outlet, flow_input_type, flow_output_type) = match &self.stages[stage_index] {
        | StageDefinition::Flow(flow) => (flow.inlet, flow.outlet, flow.input_type, flow.output_type),
        | _ => continue,
      };

      let mut consumed_input = false;
      let mut outputs = Vec::new();

      if let Some((edge_index, input)) = self.poll_from_incoming_edges(flow_inlet)? {
        consumed_input = true;
        if input.as_ref().type_id() != flow_input_type {
          return Err(StreamError::TypeMismatch);
        }

        outputs = {
          let StageDefinition::Flow(flow) = &mut self.stages[stage_index] else {
            return Err(StreamError::InvalidConnection);
          };
          flow.logic.apply_with_edge(edge_index, input)?
        };
      }

      if outputs.is_empty() {
        outputs = {
          let StageDefinition::Flow(flow) = &mut self.stages[stage_index] else {
            return Err(StreamError::InvalidConnection);
          };
          flow.logic.drain_pending()?
        };
      }

      if consumed_input {
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
    if !self.demand.has_demand() {
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

    let decision = {
      let StageDefinition::Sink(sink) = &mut self.stages[self.sink_index] else {
        return Err(StreamError::InvalidConnection);
      };
      sink.logic.on_push(value, &mut self.demand)?
    };
    match decision {
      | SinkDecision::Continue => Ok(true),
      | SinkDecision::Complete => {
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
