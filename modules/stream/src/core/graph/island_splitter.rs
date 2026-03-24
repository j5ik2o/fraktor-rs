//! Island splitting logic for async boundaries in stream graphs.
//!
//! Splits a `StreamPlan` into multiple independently executable islands
//! based on per-stage async boundary attributes. This is the foundation
//! for Pekko-compatible multi-island materialization.

use alloc::{boxed::Box, collections::VecDeque, string::String, vec, vec::Vec};
use core::any::TypeId;

use super::{island_boundary::IslandBoundaryShared, shape::PortId};
use crate::core::{
  Attributes, DispatcherAttribute, InputBuffer, MatCombine, SinkDefinition, SourceDefinition, StageDefinition,
  StreamPlan, StreamPlanEdge, SupervisionStrategy, stage::StageKind,
};

#[cfg(test)]
mod tests;

/// Unique identifier for an island within an `IslandPlan`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct IslandId(usize);

impl IslandId {
  /// Returns the underlying index.
  #[must_use]
  pub(crate) const fn as_usize(self) -> usize {
    self.0
  }
}

/// A single island: a subset of stages from the original plan that
/// execute together in one interpreter / mailbox.
#[allow(dead_code)]
pub(crate) struct SingleIslandPlan {
  id:             IslandId,
  stages:         Vec<StageDefinition>,
  edges:          Vec<StreamPlanEdge>,
  source_indices: Vec<usize>,
  sink_indices:   Vec<usize>,
  flow_order:     Vec<usize>,
  dispatcher:     Option<String>,
}

#[allow(dead_code)]
impl SingleIslandPlan {
  /// Returns the island identifier.
  #[must_use]
  pub(crate) const fn id(&self) -> IslandId {
    self.id
  }

  /// Returns the number of stages in this island.
  #[must_use]
  pub(crate) const fn stage_count(&self) -> usize {
    self.stages.len()
  }

  /// Returns the dispatcher name for this island, if any.
  #[must_use]
  pub(crate) fn dispatcher(&self) -> Option<&str> {
    self.dispatcher.as_deref()
  }

  /// Returns the source stage indices within this island.
  #[must_use]
  pub(crate) fn source_indices(&self) -> &[usize] {
    &self.source_indices
  }

  /// Returns the sink stage indices within this island.
  #[must_use]
  pub(crate) fn sink_indices(&self) -> &[usize] {
    &self.sink_indices
  }

  #[must_use]
  pub(crate) fn input_buffer_capacity_for_inlet(&self, inlet: PortId) -> Option<usize> {
    self.stages.iter().find_map(|stage| {
      let stage_inlet = stage.inlet()?;
      if stage_inlet != inlet {
        return None;
      }
      stage.attributes().get::<InputBuffer>().map(|buffer| buffer.max)
    })
  }

  /// Adds a boundary sink stage that receives from the given upstream outlet port
  /// and pushes into the shared boundary buffer.
  pub(crate) fn add_boundary_sink(
    &mut self,
    boundary: IslandBoundaryShared,
    upstream_outlet: PortId,
    element_type: TypeId,
  ) {
    use super::boundary_sink_logic::BoundarySinkLogic;

    let inlet = PortId::next();
    let idx = self.stages.len();
    self.stages.push(StageDefinition::Sink(SinkDefinition {
      kind: StageKind::Custom,
      inlet,
      input_type: element_type,
      mat_combine: MatCombine::KeepLeft,
      supervision: SupervisionStrategy::Stop,
      restart: None,
      logic: Box::new(BoundarySinkLogic::new(boundary)),
      attributes: Attributes::new(),
    }));
    self.sink_indices.push(idx);
    self.edges.push(StreamPlanEdge { from_port: upstream_outlet, to_port: inlet, mat: MatCombine::KeepLeft });
  }

  /// Adds a boundary source stage that pulls from the shared boundary buffer
  /// and feeds the given downstream inlet port.
  pub(crate) fn add_boundary_source(
    &mut self,
    boundary: IslandBoundaryShared,
    downstream_inlet: PortId,
    element_type: TypeId,
  ) {
    use super::boundary_source_logic::BoundarySourceLogic;

    let outlet = PortId::next();
    let idx = self.stages.len();
    self.stages.push(StageDefinition::Source(SourceDefinition {
      kind: StageKind::Custom,
      outlet,
      output_type: element_type,
      mat_combine: MatCombine::KeepLeft,
      supervision: SupervisionStrategy::Stop,
      restart: None,
      logic: Box::new(BoundarySourceLogic::new(boundary)),
      attributes: Attributes::new(),
    }));
    self.source_indices.push(idx);
    self.edges.push(StreamPlanEdge { from_port: outlet, to_port: downstream_inlet, mat: MatCombine::KeepLeft });
  }

  /// Converts this island into a `StreamPlan`.
  ///
  /// This bypasses `StreamPlan::from_parts()` validation because the plan
  /// was already validated before splitting.
  pub(crate) fn into_stream_plan(self) -> StreamPlan {
    StreamPlan::from_raw_parts(self.stages, self.edges, self.source_indices, self.sink_indices, self.flow_order)
  }
}

/// An edge that crosses an island boundary.
#[allow(dead_code)]
pub(crate) struct IslandCrossing {
  upstream_island:   IslandId,
  downstream_island: IslandId,
  upstream_port:     PortId,
  downstream_port:   PortId,
  mat:               MatCombine,
  /// The `TypeId` of the values flowing through this crossing.
  element_type:      TypeId,
}

#[allow(clippy::wrong_self_convention, dead_code)]
impl IslandCrossing {
  /// Returns the upstream island.
  #[must_use]
  pub(crate) const fn from_island(&self) -> IslandId {
    self.upstream_island
  }

  /// Returns the downstream island.
  #[must_use]
  pub(crate) const fn to_island(&self) -> IslandId {
    self.downstream_island
  }

  /// Returns the upstream port.
  #[must_use]
  pub(crate) const fn from_port(&self) -> PortId {
    self.upstream_port
  }

  /// Returns the downstream port.
  #[must_use]
  pub(crate) const fn to_port(&self) -> PortId {
    self.downstream_port
  }

  /// Returns the materialization combine rule for the crossing edge.
  #[must_use]
  pub(crate) const fn mat(&self) -> MatCombine {
    self.mat
  }

  /// Returns the `TypeId` of the elements flowing through this crossing.
  #[must_use]
  pub(crate) const fn element_type(&self) -> TypeId {
    self.element_type
  }
}

/// Result of splitting a `StreamPlan` into islands.
pub(crate) struct IslandPlan {
  islands:   Vec<SingleIslandPlan>,
  crossings: Vec<IslandCrossing>,
}

impl IslandPlan {
  /// Returns the list of islands.
  #[must_use]
  pub(crate) fn islands(&self) -> &[SingleIslandPlan] {
    &self.islands
  }

  /// Returns the list of cross-island edges.
  #[must_use]
  #[allow(dead_code)]
  pub(crate) fn crossings(&self) -> &[IslandCrossing] {
    &self.crossings
  }

  /// Converts a single-island plan back into a `StreamPlan`.
  ///
  /// # Panics
  ///
  /// Panics if the plan contains more than one island.
  pub(crate) fn into_single_plan(mut self) -> StreamPlan {
    assert!(self.islands.len() == 1, "into_single_plan called with {} islands", self.islands.len());
    self.islands.remove(0).into_stream_plan()
  }

  /// Consumes this plan and returns the islands and crossings.
  pub(crate) fn into_parts(self) -> (Vec<SingleIslandPlan>, Vec<IslandCrossing>) {
    (self.islands, self.crossings)
  }
}

/// Splits a `StreamPlan` into islands at async boundary markers.
///
/// Semantics: a stage with `is_async()` attribute is the **last** stage
/// in its current island. The next stage in topological order starts a
/// new island.
pub(crate) struct IslandSplitter;

type PortStageMap = Vec<(PortId, usize)>;

impl IslandSplitter {
  fn port_stage_maps(plan: &StreamPlan) -> (PortStageMap, PortStageMap) {
    let mut outlet_to_stage = Vec::new();
    let mut inlet_to_stage = Vec::new();
    for (stage_idx, stage) in plan.stages.iter().enumerate() {
      if let Some(outlet) = stage.outlet() {
        outlet_to_stage.push((outlet, stage_idx));
      }
      if let Some(inlet) = stage.inlet() {
        inlet_to_stage.push((inlet, stage_idx));
      }
    }
    (outlet_to_stage, inlet_to_stage)
  }

  fn topological_stage_order(plan: &StreamPlan) -> Vec<usize> {
    let stage_count = plan.stages.len();
    let (outlet_to_stage, inlet_to_stage) = Self::port_stage_maps(plan);

    let mut incoming = vec![0_usize; stage_count];
    let mut adjacency = vec![Vec::new(); stage_count];
    for edge in &plan.edges {
      let Some(from_stage) = outlet_to_stage.iter().find(|(port, _)| *port == edge.from_port).map(|(_, idx)| *idx)
      else {
        continue;
      };
      let Some(to_stage) = inlet_to_stage.iter().find(|(port, _)| *port == edge.to_port).map(|(_, idx)| *idx) else {
        continue;
      };
      incoming[to_stage] = incoming[to_stage].saturating_add(1);
      adjacency[from_stage].push(to_stage);
    }

    let mut ready = VecDeque::new();
    for (stage_index, count) in incoming.iter().enumerate() {
      if *count == 0 {
        ready.push_back(stage_index);
      }
    }

    let mut ordered = Vec::with_capacity(stage_count);
    while let Some(stage_index) = ready.pop_front() {
      ordered.push(stage_index);
      for next_index in &adjacency[stage_index] {
        incoming[*next_index] = incoming[*next_index].saturating_sub(1);
        if incoming[*next_index] == 0 {
          ready.push_back(*next_index);
        }
      }
    }

    if ordered.len() == stage_count { ordered } else { (0..stage_count).collect() }
  }

  fn assign_islands(plan: &StreamPlan) -> (Vec<usize>, usize, Vec<Option<String>>) {
    let stage_count = plan.stages.len();
    let (outlet_to_stage, inlet_to_stage) = Self::port_stage_maps(plan);
    let mut adjacency = vec![Vec::new(); stage_count];
    let mut dispatcher_candidates = vec![Vec::<String>::new(); stage_count];

    for edge in &plan.edges {
      let Some(from_stage) = outlet_to_stage.iter().find(|(port, _)| *port == edge.from_port).map(|(_, idx)| *idx)
      else {
        continue;
      };
      let Some(to_stage) = inlet_to_stage.iter().find(|(port, _)| *port == edge.to_port).map(|(_, idx)| *idx) else {
        continue;
      };

      if plan.stages[from_stage].attributes().is_async() {
        if let Some(dispatcher) = plan.stages[from_stage].attributes().get::<DispatcherAttribute>() {
          dispatcher_candidates[to_stage].push(String::from(dispatcher.name()));
        }
        continue;
      }

      adjacency[from_stage].push(to_stage);
      adjacency[to_stage].push(from_stage);
    }

    let mut component = vec![usize::MAX; stage_count];
    let mut component_count = 0_usize;
    for start in 0..stage_count {
      if component[start] != usize::MAX {
        continue;
      }
      let mut queue = VecDeque::new();
      queue.push_back(start);
      component[start] = component_count;
      while let Some(stage_index) = queue.pop_front() {
        for next in &adjacency[stage_index] {
          if component[*next] == usize::MAX {
            component[*next] = component_count;
            queue.push_back(*next);
          }
        }
      }
      component_count = component_count.saturating_add(1);
    }

    let topo_order = Self::topological_stage_order(plan);
    let mut component_to_island = vec![usize::MAX; component_count];
    let mut stage_island = vec![usize::MAX; stage_count];
    let mut next_island = 0_usize;
    for stage_index in topo_order {
      let component_id = component[stage_index];
      if component_to_island[component_id] == usize::MAX {
        component_to_island[component_id] = next_island;
        next_island = next_island.saturating_add(1);
      }
      stage_island[stage_index] = component_to_island[component_id];
    }

    let mut dispatcher_for_island = vec![None; next_island];
    for (stage_index, names) in dispatcher_candidates.iter().enumerate() {
      let island_id = stage_island[stage_index];
      if names.is_empty() || dispatcher_for_island[island_id].is_some() {
        continue;
      }
      dispatcher_for_island[island_id] = names.first().cloned();
    }

    (stage_island, next_island, dispatcher_for_island)
  }

  /// Splits the given plan into islands.
  pub(crate) fn split(plan: StreamPlan) -> IslandPlan {
    let (stage_island, island_count, mut dispatcher_for_island) = Self::assign_islands(&plan);
    let (outlet_to_stage, inlet_to_stage) = Self::port_stage_maps(&plan);

    // Classify edges as internal or crossing
    let mut island_edges: Vec<Vec<StreamPlanEdge>> = (0..island_count).map(|_| Vec::new()).collect();
    let mut crossings = Vec::new();

    for edge in &plan.edges {
      let from_stage = outlet_to_stage.iter().find(|(port, _)| *port == edge.from_port).map(|(_, idx)| *idx);
      let to_stage = inlet_to_stage.iter().find(|(port, _)| *port == edge.to_port).map(|(_, idx)| *idx);

      if let (Some(from_idx), Some(to_idx)) = (from_stage, to_stage) {
        let from_isl = stage_island[from_idx];
        let to_isl = stage_island[to_idx];

        if from_isl == to_isl {
          island_edges[from_isl].push(StreamPlanEdge {
            from_port: edge.from_port,
            to_port:   edge.to_port,
            mat:       edge.mat,
          });
        } else {
          // Get element type from the upstream stage's output_type
          let element_type = plan.stages[from_idx].output_type().unwrap_or_else(TypeId::of::<()>);
          crossings.push(IslandCrossing {
            upstream_island: IslandId(from_isl),
            downstream_island: IslandId(to_isl),
            upstream_port: edge.from_port,
            downstream_port: edge.to_port,
            mat: edge.mat,
            element_type,
          });
        }
      }
    }

    // Distribute stages into islands
    let mut island_stages: Vec<Vec<StageDefinition>> = (0..island_count).map(|_| Vec::new()).collect();

    for (original_idx, stage) in plan.stages.into_iter().enumerate() {
      let isl = stage_island[original_idx];
      island_stages[isl].push(stage);
    }

    // Build SingleIslandPlan for each island
    let mut islands = Vec::with_capacity(island_count);

    for (isl_idx, stages) in island_stages.into_iter().enumerate() {
      let mut source_indices = Vec::new();
      let mut sink_indices = Vec::new();
      let mut flow_order = Vec::new();

      for (local_idx, stage) in stages.iter().enumerate() {
        match stage {
          | StageDefinition::Source(_) => source_indices.push(local_idx),
          | StageDefinition::Flow(_) => flow_order.push(local_idx),
          | StageDefinition::Sink(_) => sink_indices.push(local_idx),
        }
      }

      let dispatcher = dispatcher_for_island[isl_idx].take();

      islands.push(SingleIslandPlan {
        id: IslandId(isl_idx),
        stages,
        edges: core::mem::take(&mut island_edges[isl_idx]),
        source_indices,
        sink_indices,
        flow_order,
        dispatcher,
      });
    }

    IslandPlan { islands, crossings }
  }
}
