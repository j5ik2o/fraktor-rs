//! Internal graph interpreter and boundary wiring.

mod boundary_sink_logic;
mod boundary_source_logic;
mod island_boundary;
mod island_splitter;

pub(in crate::core) use island_boundary::{DEFAULT_BOUNDARY_CAPACITY, IslandBoundaryShared};
pub(in crate::core) use island_splitter::IslandSplitter;
