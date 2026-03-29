//! Internal graph interpreter and boundary wiring.

use crate::core::graph::{
  DEFAULT_BOUNDARY_CAPACITY as GRAPH_DEFAULT_BOUNDARY_CAPACITY, GraphInterpreter as GraphInterpreterInner,
  GraphStageFlowAdapter as GraphStageFlowAdapterInner, IslandBoundaryShared as IslandBoundarySharedInner,
  IslandSplitter as IslandSplitterInner,
};

pub(in crate::core) const DEFAULT_BOUNDARY_CAPACITY: usize = GRAPH_DEFAULT_BOUNDARY_CAPACITY;
pub(in crate::core) type GraphInterpreter = GraphInterpreterInner;
pub(in crate::core) type GraphStageFlowAdapter<In, Out, Mat> = GraphStageFlowAdapterInner<In, Out, Mat>;
pub(in crate::core) type IslandBoundaryShared = IslandBoundarySharedInner;
pub(in crate::core) type IslandSplitter = IslandSplitterInner;
