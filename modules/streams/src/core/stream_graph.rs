use alloc::vec::Vec;

#[cfg(test)]
mod tests;

use super::{
  FlowDefinition, Inlet, MatCombine, Outlet, PortId, SourceDefinition, StageDefinition, StageKind, StreamError,
  StreamPlan,
};

/// Immutable blueprint that stores stage connectivity.
///
/// This type only captures stage definitions and their port wiring.
/// Runtime buffers and drive state are allocated later during materialization.
pub struct StreamGraph {
  nodes:        Vec<GraphNode>,
  edges:        Vec<GraphEdge>,
  ports:        Vec<PortId>,
  next_node_id: usize,
}

impl StreamGraph {
  /// Creates an empty graph.
  #[must_use]
  pub const fn new() -> Self {
    Self { nodes: Vec::new(), edges: Vec::new(), ports: Vec::new(), next_node_id: 0 }
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
    if let Some(existing) = self.edges.iter().find(|edge| edge.from == from && edge.to == to) {
      let _ = existing.mat;
      return Err(StreamError::InvalidConnection);
    }
    self.edges.push(GraphEdge { from, to, mat: combine });
    Ok(())
  }

  pub(super) fn push_stage(&mut self, stage: StageDefinition) {
    if let Some(inlet) = stage.inlet() {
      self.ports.push(inlet);
    }
    if let Some(outlet) = stage.outlet() {
      self.ports.push(outlet);
    }
    self.nodes.push(GraphNode { id: self.next_node_id, stage });
    self.next_node_id = self.next_node_id.saturating_add(1);
  }

  pub(super) fn append(&mut self, mut other: StreamGraph) {
    if self.nodes.is_empty() {
      self.nodes = other.nodes;
      self.edges = other.edges;
      self.ports = other.ports;
      self.next_node_id = other.next_node_id;
      return;
    }
    if other.nodes.is_empty() {
      return;
    }
    if let (Some(from), Some(to)) = (self.tail_outlet(), other.head_inlet()) {
      self.edges.push(GraphEdge { from, to, mat: MatCombine::KeepLeft });
    }
    let offset = self.next_node_id;
    for node in &mut other.nodes {
      node.id = node.id.saturating_add(offset);
    }
    self.next_node_id = self.next_node_id.saturating_add(other.next_node_id);
    self.ports.append(&mut other.ports);
    self.edges.append(&mut other.edges);
    self.nodes.append(&mut other.nodes);
  }

  pub(super) fn into_plan(self) -> Result<StreamPlan, StreamError> {
    if self.nodes.is_empty() {
      return Err(StreamError::InvalidConnection);
    }
    let mut stages = Vec::with_capacity(self.nodes.len());
    let mut source_count = 0_usize;
    let mut sink_count = 0_usize;
    for node in self.nodes {
      Self::ensure_stage_metadata(&node.stage)?;
      match node.stage {
        | StageDefinition::Source(_) => source_count = source_count.saturating_add(1),
        | StageDefinition::Sink(_) => sink_count = sink_count.saturating_add(1),
        | StageDefinition::Flow(_) => {},
      }
      stages.push(node.stage);
    }
    if source_count != 1 || sink_count != 1 {
      return Err(StreamError::InvalidConnection);
    }
    let edges = self.edges.into_iter().map(|edge| (edge.from, edge.to, edge.mat)).collect();
    Ok(StreamPlan::from_parts(stages, edges))
  }

  pub(super) fn into_source_parts(self) -> Result<(SourceDefinition, Vec<FlowDefinition>), StreamError> {
    let mut iter = self.nodes.into_iter().map(|node| node.stage);
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
        matches!(
          kind,
          StageKind::FlowMap
            | StageKind::FlowFlatMapConcat
            | StageKind::FlowFlatMapMerge
            | StageKind::FlowBuffer
            | StageKind::FlowAsyncBoundary
            | StageKind::FlowBroadcast
            | StageKind::FlowBalance
            | StageKind::FlowMerge
            | StageKind::FlowZip
            | StageKind::FlowConcat
            | StageKind::Custom
        )
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
    self.nodes.first().and_then(|node| node.stage.inlet())
  }

  pub(super) fn tail_outlet(&self) -> Option<PortId> {
    self.nodes.last().and_then(|node| node.stage.outlet())
  }
}

impl Default for StreamGraph {
  fn default() -> Self {
    Self::new()
  }
}

struct GraphNode {
  id:    usize,
  stage: StageDefinition,
}

#[derive(Clone, Copy)]
struct GraphEdge {
  from: PortId,
  to:   PortId,
  mat:  MatCombine,
}
