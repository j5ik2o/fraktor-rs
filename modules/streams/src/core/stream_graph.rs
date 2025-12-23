use alloc::vec::Vec;

#[cfg(test)]
mod tests;

use super::{
  Connection, FlowDefinition, Inlet, MatCombine, Outlet, PortId, SourceDefinition, StageDefinition, StageKind,
  StreamError, StreamPlan,
};

/// Graph that stores stage connectivity.
pub struct StreamGraph {
  stages:      Vec<StageDefinition>,
  connections: Vec<Connection>,
  ports:       Vec<PortId>,
}

impl StreamGraph {
  /// Creates an empty graph.
  #[must_use]
  pub const fn new() -> Self {
    Self { stages: Vec::new(), connections: Vec::new(), ports: Vec::new() }
  }

  /// Connects two ports with type safety.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::InvalidConnection`] when a port is unknown.
  pub fn connect<T>(
    &mut self,
    upstream: &Outlet<T>,
    downstream: &Inlet<T>,
    combine: MatCombine,
  ) -> Result<(), StreamError> {
    let from = upstream.id();
    let to = downstream.id();
    if !self.has_port(from) || !self.has_port(to) {
      return Err(StreamError::InvalidConnection);
    }
    if let Some(existing) = self.connections.iter().find(|conn| conn.from == from && conn.to == to) {
      let _ = existing.mat;
      return Err(StreamError::InvalidConnection);
    }
    self.connections.push(Connection { from, to, mat: combine });
    Ok(())
  }

  pub(super) fn push_stage(&mut self, stage: StageDefinition) {
    if let Some(inlet) = stage.inlet() {
      self.ports.push(inlet);
    }
    if let Some(outlet) = stage.outlet() {
      self.ports.push(outlet);
    }
    self.stages.push(stage);
  }

  pub(super) fn append(&mut self, mut other: StreamGraph) {
    if self.stages.is_empty() {
      self.stages = other.stages;
      self.connections = other.connections;
      self.ports = other.ports;
      return;
    }
    if other.stages.is_empty() {
      return;
    }
    if let (Some(from), Some(to)) = (self.tail_outlet(), other.head_inlet()) {
      self.connections.push(Connection { from, to, mat: MatCombine::KeepLeft });
    }
    self.ports.append(&mut other.ports);
    self.connections.append(&mut other.connections);
    self.stages.append(&mut other.stages);
  }

  pub(super) fn into_plan(self) -> Result<StreamPlan, StreamError> {
    let mut iter = self.stages.into_iter();
    let source = match iter.next() {
      | Some(stage) => {
        Self::ensure_stage_metadata(&stage)?;
        match stage {
          | StageDefinition::Source(definition) => definition,
          | _ => return Err(StreamError::InvalidConnection),
        }
      },
      | None => return Err(StreamError::InvalidConnection),
    };
    let mut flows = Vec::new();
    let mut sink = None;
    for stage in iter {
      Self::ensure_stage_metadata(&stage)?;
      match stage {
        | StageDefinition::Flow(definition) => flows.push(definition),
        | StageDefinition::Sink(definition) => {
          if sink.is_some() {
            return Err(StreamError::InvalidConnection);
          }
          sink = Some(definition);
        },
        | StageDefinition::Source(_) => return Err(StreamError::InvalidConnection),
      }
    }
    let Some(sink) = sink else {
      return Err(StreamError::InvalidConnection);
    };
    Ok(StreamPlan { source, flows, sink })
  }

  pub(super) fn into_source_parts(self) -> Result<(SourceDefinition, Vec<FlowDefinition>), StreamError> {
    let mut iter = self.stages.into_iter();
    let source = match iter.next() {
      | Some(stage) => {
        Self::ensure_stage_metadata(&stage)?;
        match stage {
          | StageDefinition::Source(definition) => definition,
          | _ => return Err(StreamError::InvalidConnection),
        }
      },
      | None => return Err(StreamError::InvalidConnection),
    };
    let mut flows = Vec::new();
    for stage in iter {
      Self::ensure_stage_metadata(&stage)?;
      match stage {
        | StageDefinition::Flow(definition) => flows.push(definition),
        | StageDefinition::Sink(_) => return Err(StreamError::InvalidConnection),
        | StageDefinition::Source(_) => return Err(StreamError::InvalidConnection),
      }
    }
    Ok((source, flows))
  }

  const fn ensure_stage_metadata(stage: &StageDefinition) -> Result<(), StreamError> {
    let kind = stage.kind();
    let _mat_combine = stage.mat_combine();
    let kind_matches = match stage {
      | StageDefinition::Source(_) => matches!(kind, StageKind::SourceSingle | StageKind::Custom),
      | StageDefinition::Flow(_) => {
        matches!(kind, StageKind::FlowMap | StageKind::FlowFlatMapConcat | StageKind::Custom)
      },
      | StageDefinition::Sink(_) => matches!(
        kind,
        StageKind::SinkIgnore
          | StageKind::SinkFold
          | StageKind::SinkHead
          | StageKind::SinkLast
          | StageKind::SinkForeach
          | StageKind::Custom
      ),
    };
    if kind_matches { Ok(()) } else { Err(StreamError::InvalidConnection) }
  }

  fn has_port(&self, port: PortId) -> bool {
    self.ports.contains(&port)
  }

  pub(super) fn head_inlet(&self) -> Option<PortId> {
    self.stages.first().and_then(StageDefinition::inlet)
  }

  pub(super) fn tail_outlet(&self) -> Option<PortId> {
    self.stages.last().and_then(StageDefinition::outlet)
  }
}

impl Default for StreamGraph {
  fn default() -> Self {
    Self::new()
  }
}
