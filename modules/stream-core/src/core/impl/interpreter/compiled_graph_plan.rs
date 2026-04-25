use alloc::vec::Vec;

use super::{buffered_edge::BufferedEdge, outlet_dispatch_state::OutletDispatchState};
use crate::core::{StageDefinition, StreamPlan, r#impl::fusing::StreamBufferConfig};

/// Stream plan compiled into interpreter-owned runtime structures.
pub(in crate::core) struct CompiledGraphPlan {
  pub(in crate::core) stages:         Vec<StageDefinition>,
  pub(in crate::core) edges:          Vec<BufferedEdge>,
  pub(in crate::core) dispatch:       Vec<OutletDispatchState>,
  pub(in crate::core) flow_order:     Vec<usize>,
  pub(in crate::core) source_indices: Vec<usize>,
  pub(in crate::core) sink_indices:   Vec<usize>,
}

impl CompiledGraphPlan {
  /// Compiles a stream plan for the graph interpreter.
  #[must_use]
  pub(in crate::core) fn compile(plan: StreamPlan, buffer_config: StreamBufferConfig) -> Self {
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
