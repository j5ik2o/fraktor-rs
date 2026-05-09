//! Internal graph interpreter and boundary wiring.

mod boundary_sink_logic;
mod boundary_source_logic;
mod buffered_edge;
mod compiled_graph_plan;
mod failure_disposition;
mod graph_connections;
pub(crate) mod graph_interpreter;
mod interpreter_snapshot_builder;
mod island_boundary;
mod island_splitter;
mod outlet_dispatch_state;

pub(crate) use island_boundary::{DEFAULT_BOUNDARY_CAPACITY, IslandBoundaryShared};
pub(crate) use island_splitter::{IslandSplitter, SingleIslandPlan};
