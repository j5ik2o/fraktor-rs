use alloc::vec::Vec;

#[cfg(test)]
mod tests;

use super::{
  MatCombine, RestartBackoff, StageDefinition, StageKind, StreamError, StreamPlan, SupervisionStrategy,
  shape::{Inlet, Outlet, PortId},
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

  pub(in crate::core) fn set_source_supervision(&mut self, supervision: SupervisionStrategy) {
    for node in &mut self.nodes {
      if let StageDefinition::Source(definition) = &mut node.stage {
        definition.supervision = supervision;
      }
    }
  }

  pub(in crate::core) fn set_source_restart(&mut self, restart: Option<RestartBackoff>) {
    for node in &mut self.nodes {
      if let StageDefinition::Source(definition) = &mut node.stage {
        definition.restart = restart;
      }
    }
  }

  pub(in crate::core) fn set_flow_supervision(&mut self, supervision: SupervisionStrategy) {
    for node in &mut self.nodes {
      if let StageDefinition::Flow(definition) = &mut node.stage {
        definition.supervision = supervision;
      }
    }
  }

  pub(in crate::core) fn set_flow_restart(&mut self, restart: Option<RestartBackoff>) {
    for node in &mut self.nodes {
      if let StageDefinition::Flow(definition) = &mut node.stage {
        definition.restart = restart;
      }
    }
  }

  pub(in crate::core) fn set_sink_supervision(&mut self, supervision: SupervisionStrategy) {
    for node in &mut self.nodes {
      if let StageDefinition::Sink(definition) = &mut node.stage {
        definition.supervision = supervision;
      }
    }
  }

  pub(in crate::core) fn set_sink_restart(&mut self, restart: Option<RestartBackoff>) {
    for node in &mut self.nodes {
      if let StageDefinition::Sink(definition) = &mut node.stage {
        definition.restart = restart;
      }
    }
  }

  pub(in crate::core) fn push_stage(&mut self, stage: StageDefinition) {
    if let Some(inlet) = stage.inlet() {
      self.ports.push(inlet);
    }
    if let Some(outlet) = stage.outlet() {
      self.ports.push(outlet);
    }
    self.nodes.push(GraphNode { id: self.next_node_id, stage });
    self.next_node_id = self.next_node_id.saturating_add(1);
  }

  pub(in crate::core) fn append(&mut self, mut other: StreamGraph) {
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

  pub(in crate::core) fn into_plan(self) -> Result<StreamPlan, StreamError> {
    if self.nodes.is_empty() {
      return Err(StreamError::InvalidConnection);
    }
    let mut stages = Vec::with_capacity(self.nodes.len());
    for node in self.nodes {
      Self::ensure_stage_metadata(&node.stage)?;
      stages.push(node.stage);
    }
    let edges = self.edges.into_iter().map(|edge| (edge.from, edge.to, edge.mat)).collect();
    StreamPlan::from_parts(stages, edges)
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
            | StageKind::FlowMapAsync
            | StageKind::FlowStatefulMap
            | StageKind::FlowStatefulMapConcat
            | StageKind::FlowMapConcat
            | StageKind::FlowMapOption
            | StageKind::FlowFilter
            | StageKind::FlowDrop
            | StageKind::FlowTake
            | StageKind::FlowDropWhile
            | StageKind::FlowTakeWhile
            | StageKind::FlowTakeUntil
            | StageKind::FlowGrouped
            | StageKind::FlowSliding
            | StageKind::FlowScan
            | StageKind::FlowIntersperse
            | StageKind::FlowFlatMapConcat
            | StageKind::FlowFlatMapMerge
            | StageKind::FlowBuffer
            | StageKind::FlowAsyncBoundary
            | StageKind::FlowThrottle
            | StageKind::FlowDelay
            | StageKind::FlowInitialDelay
            | StageKind::FlowTakeWithin
            | StageKind::FlowBatch
            | StageKind::FlowGroupBy
            | StageKind::FlowRecover
            | StageKind::FlowRecoverWithRetries
            | StageKind::FlowSplitWhen
            | StageKind::FlowSplitAfter
            | StageKind::FlowMergeSubstreams
            | StageKind::FlowMergeSubstreamsWithParallelism
            | StageKind::FlowConcatSubstreams
            | StageKind::FlowPartition
            | StageKind::FlowUnzip
            | StageKind::FlowUnzipWith
            | StageKind::FlowBroadcast
            | StageKind::FlowBalance
            | StageKind::FlowMerge
            | StageKind::FlowInterleave
            | StageKind::FlowPrepend
            | StageKind::FlowZip
            | StageKind::FlowZipAll
            | StageKind::FlowZipWithIndex
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

  pub(in crate::core) fn head_inlet(&self) -> Option<PortId> {
    self.nodes.first().and_then(|node| node.stage.inlet())
  }

  pub(in crate::core) fn tail_outlet(&self) -> Option<PortId> {
    self.nodes.last().and_then(|node| node.stage.outlet())
  }

  pub(in crate::core) fn expected_fan_out_for_outlet(&self, outlet: PortId) -> Option<usize> {
    for node in &self.nodes {
      if let StageDefinition::Flow(definition) = &node.stage
        && definition.outlet == outlet
      {
        return definition.logic.expected_fan_out();
      }
    }
    None
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
