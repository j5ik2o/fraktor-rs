use alloc::{string::String, vec::Vec};

use fraktor_utils_core_rs::sync::ArcShared;

#[cfg(test)]
#[path = "stream_graph_test.rs"]
mod tests;

use crate::{
  KillSwitchStateHandle, StageDefinition, StageKind, StreamError, StreamPlan, SupervisionStrategy,
  attributes::Attributes,
  r#impl::RestartBackoff,
  materialization::MatCombine,
  shape::{Inlet, Outlet, PortId},
};

/// Immutable blueprint that stores stage connectivity.
///
/// This type only captures stage definitions and their port wiring.
/// Runtime buffers and drive state are allocated later during materialization.
pub(crate) struct StreamGraph {
  nodes:              Vec<GraphNode>,
  edges:              Vec<GraphEdge>,
  ports:              Vec<PortId>,
  attributes:         Attributes,
  kill_switch_states: Vec<KillSwitchStateHandle>,
  next_node_id:       usize,
}

impl StreamGraph {
  /// Creates an empty graph.
  #[must_use]
  pub(crate) const fn new() -> Self {
    Self {
      nodes:              Vec::new(),
      edges:              Vec::new(),
      ports:              Vec::new(),
      attributes:         Attributes::new(),
      kill_switch_states: Vec::new(),
      next_node_id:       0,
    }
  }

  /// Connects two ports with type safety.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::InvalidConnection`] when a port is unknown.
  pub(crate) fn connect<T>(
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

  /// Connects two ports that are guaranteed to be registered by this builder.
  ///
  /// # Panics
  ///
  /// Panics if either port is unknown — this indicates a programming bug.
  #[allow(clippy::expect_used)]
  pub(crate) fn connect_or_panic<T>(&mut self, upstream: &Outlet<T>, downstream: &Inlet<T>, combine: MatCombine) {
    self.connect(upstream, downstream, combine).expect("internal: ports registered by this builder");
  }

  pub(crate) fn set_source_supervision(&mut self, supervision: SupervisionStrategy) {
    for node in &mut self.nodes {
      if let StageDefinition::Source(definition) = &mut node.stage {
        definition.supervision = supervision;
      }
    }
  }

  pub(crate) fn set_source_restart(&mut self, restart: &Option<RestartBackoff>) {
    for node in &mut self.nodes {
      if let StageDefinition::Source(definition) = &mut node.stage {
        definition.restart.clone_from(restart);
      }
    }
  }

  pub(crate) fn set_flow_supervision(&mut self, supervision: SupervisionStrategy) {
    for node in &mut self.nodes {
      if let StageDefinition::Flow(definition) = &mut node.stage {
        if definition.kind == StageKind::FlowKillSwitch {
          continue;
        }
        definition.supervision = supervision;
      }
    }
  }

  pub(crate) fn set_flow_restart(&mut self, restart: &Option<RestartBackoff>) {
    for node in &mut self.nodes {
      if let StageDefinition::Flow(definition) = &mut node.stage {
        if definition.kind == StageKind::FlowKillSwitch {
          continue;
        }
        definition.restart.clone_from(restart);
      }
    }
  }

  pub(crate) fn set_sink_supervision(&mut self, supervision: SupervisionStrategy) {
    for node in &mut self.nodes {
      if let StageDefinition::Sink(definition) = &mut node.stage {
        definition.supervision = supervision;
      }
    }
  }

  pub(crate) fn set_sink_restart(&mut self, restart: &Option<RestartBackoff>) {
    for node in &mut self.nodes {
      if let StageDefinition::Sink(definition) = &mut node.stage {
        definition.restart.clone_from(restart);
      }
    }
  }

  pub(crate) fn push_stage(&mut self, stage: StageDefinition) {
    if let Some(inlet) = stage.inlet() {
      self.ports.push(inlet);
    }
    if let Some(outlet) = stage.outlet() {
      self.ports.push(outlet);
    }
    self.nodes.push(GraphNode { id: self.next_node_id, stage, attributes: Attributes::new() });
    self.next_node_id = self.next_node_id.saturating_add(1);
  }

  /// Marks the last node in this graph with an async boundary attribute.
  ///
  /// This is a no-op if the graph is empty.
  pub(crate) fn mark_last_node_async(&mut self) {
    if let Some(node) = self.nodes.last_mut() {
      let old = core::mem::take(&mut node.attributes);
      node.attributes = old.and(Attributes::async_boundary());
    }
  }

  /// Marks the last node with both async boundary and dispatcher attributes.
  ///
  /// This is a no-op if the graph is empty.
  pub(crate) fn mark_last_node_dispatcher(&mut self, name: impl Into<String>) {
    if let Some(node) = self.nodes.last_mut() {
      let old = core::mem::take(&mut node.attributes);
      node.attributes = old.and(Attributes::async_boundary().and(Attributes::dispatcher(name)));
    }
  }

  pub(crate) fn append(&mut self, other: StreamGraph) {
    let connection = match (self.tail_outlet(), other.head_inlet()) {
      | (Some(from), Some(to)) => Some((from, to)),
      | _ => None,
    };
    self.append_unwired(other);
    if let Some((from, to)) = connection {
      let upstream = Outlet::<()>::from_id(from);
      let downstream = Inlet::<()>::from_id(to);
      assert!(self.connect(&upstream, &downstream, MatCombine::Left).is_ok(), "stream graph appends only known ports");
    }
  }

  pub(crate) fn append_unwired(&mut self, mut other: StreamGraph) {
    let other_kill_switch_states = core::mem::take(&mut other.kill_switch_states);
    if self.nodes.is_empty() {
      self.nodes = other.nodes;
      self.edges = other.edges;
      self.ports = other.ports;
      self.attributes = other.attributes;
      self.kill_switch_states = other_kill_switch_states;
      self.next_node_id = other.next_node_id;
      return;
    }
    if other.nodes.is_empty() {
      let old_attrs = core::mem::take(&mut self.attributes);
      self.attributes = old_attrs.and(other.attributes);
      self.merge_kill_switch_states(other_kill_switch_states);
      return;
    }
    let offset = self.next_node_id;
    for node in &mut other.nodes {
      node.id = node.id.saturating_add(offset);
    }
    self.next_node_id = self.next_node_id.saturating_add(other.next_node_id);
    self.ports.append(&mut other.ports);
    self.edges.append(&mut other.edges);
    self.nodes.append(&mut other.nodes);
    let old_attrs = core::mem::take(&mut self.attributes);
    self.attributes = old_attrs.and(other.attributes);
    self.merge_kill_switch_states(other_kill_switch_states);
  }

  pub(crate) fn into_plan(self) -> Result<StreamPlan, StreamError> {
    if self.nodes.is_empty() {
      return Err(StreamError::InvalidConnection);
    }
    let mut stages = Vec::with_capacity(self.nodes.len());
    for node in self.nodes {
      Self::ensure_stage_metadata(&node.stage)?;
      if node.attributes.is_empty() {
        stages.push(node.stage);
      } else {
        stages.push(node.stage.with_attributes(node.attributes));
      }
    }
    let edges = self.edges.into_iter().map(|edge| (edge.from, edge.to, edge.mat)).collect();
    let mut plan = StreamPlan::from_parts(stages, edges)?;
    for kill_switch_state in self.kill_switch_states {
      plan = plan.with_shared_kill_switch_state(kill_switch_state);
    }
    Ok(plan)
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
            | StageKind::FlowStatefulMapWithOnComplete
            | StageKind::FlowStatefulMapConcatAccumulator
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
            | StageKind::FlowBackpressureTimeout
            | StageKind::FlowCompletionTimeout
            | StageKind::FlowIdleTimeout
            | StageKind::FlowInitialTimeout
            | StageKind::FlowBatch
            | StageKind::FlowGroupBy
            | StageKind::FlowLog
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
            | StageKind::FlowMergePreferred
            | StageKind::FlowMergePrioritized
            | StageKind::FlowMergeSorted
            | StageKind::FlowMergeLatest
            | StageKind::FlowInterleave
            | StageKind::FlowPrepend
            | StageKind::FlowZip
            | StageKind::FlowZipAll
            | StageKind::FlowZipWithIndex
            | StageKind::FlowConcat
            | StageKind::FlowKillSwitch
            | StageKind::FlowWatchTermination
            | StageKind::FlowDebounce
            | StageKind::FlowSample
            | StageKind::FlowWireTap
            | StageKind::FlowKeepAlive
            | StageKind::FlowSwitchMap
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

  pub(crate) fn head_inlet(&self) -> Option<PortId> {
    self.nodes.first().and_then(|node| node.stage.inlet())
  }

  pub(crate) fn tail_outlet(&self) -> Option<PortId> {
    self.nodes.last().and_then(|node| node.stage.outlet())
  }

  pub(crate) fn into_stages(self) -> Vec<StageDefinition> {
    self.nodes.into_iter().map(|node| node.stage).collect()
  }

  pub(crate) fn set_attributes(&mut self, attributes: Attributes) {
    self.attributes = attributes;
  }

  pub(crate) fn add_attributes(&mut self, attributes: Attributes) {
    let old_attrs = core::mem::take(&mut self.attributes);
    self.attributes = old_attrs.and(attributes);
  }

  pub(crate) fn set_shared_kill_switch_state(&mut self, kill_switch_state: KillSwitchStateHandle) {
    if self.kill_switch_states.iter().any(|existing| ArcShared::ptr_eq(existing, &kill_switch_state)) {
      return;
    }
    self.kill_switch_states.push(kill_switch_state);
  }

  pub(crate) fn expected_fan_out_for_outlet(&self, outlet: PortId) -> Option<usize> {
    for node in &self.nodes {
      if let StageDefinition::Flow(definition) = &node.stage
        && definition.outlet == outlet
      {
        return definition.logic.expected_fan_out();
      }
    }
    None
  }

  fn merge_kill_switch_states(&mut self, kill_switch_states: Vec<KillSwitchStateHandle>) {
    for kill_switch_state in kill_switch_states {
      self.set_shared_kill_switch_state(kill_switch_state);
    }
  }
}

impl Default for StreamGraph {
  fn default() -> Self {
    Self::new()
  }
}

struct GraphNode {
  id:         usize,
  stage:      StageDefinition,
  attributes: Attributes,
}

#[derive(Clone, Copy)]
struct GraphEdge {
  from: PortId,
  to:   PortId,
  mat:  MatCombine,
}
