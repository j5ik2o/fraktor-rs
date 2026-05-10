use alloc::vec::Vec;

use super::{buffered_edge::BufferedEdge, outlet_dispatch_state::OutletDispatchState};
use crate::{StageDefinition, StreamPlan, r#impl::fusing::StreamBufferConfig};

/// Stream plan compiled into interpreter-owned runtime structures.
pub(crate) struct CompiledGraphPlan {
  pub(crate) stages:         Vec<StageDefinition>,
  pub(crate) edges:          Vec<BufferedEdge>,
  pub(crate) dispatch:       Vec<OutletDispatchState>,
  pub(crate) flow_order:     Vec<usize>,
  pub(crate) source_indices: Vec<usize>,
  pub(crate) sink_indices:   Vec<usize>,
}

impl CompiledGraphPlan {
  /// Compiles a stream plan for the graph interpreter.
  #[must_use]
  pub(crate) fn compile(plan: StreamPlan, buffer_config: StreamBufferConfig) -> Self {
    let StreamPlan { stages, edges, source_indices, sink_indices, flow_order, .. } = plan;

    let mut buffered_edges = Vec::new();
    for edge in edges {
      buffered_edges.push(BufferedEdge::new(edge.from_port, edge.to_port, edge.mat, buffer_config));
    }

    let dispatch = Self::create_dispatch_states(&stages);

    Self { stages, edges: buffered_edges, dispatch, flow_order, source_indices, sink_indices }
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
}
