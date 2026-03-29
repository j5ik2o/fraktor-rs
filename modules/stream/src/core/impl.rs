//! Internal implementation packages mirroring Pekko's `impl` boundary.

pub(in crate::core) mod fusing;
mod hub;
mod interpreter;
mod io;
mod materialization;
mod queue;
mod streamref;

pub(in crate::core) use interpreter::{
  DEFAULT_BOUNDARY_CAPACITY, GraphInterpreter, GraphStageFlowAdapter, IslandBoundaryShared, IslandSplitter,
};

#[cfg(test)]
mod tests;
