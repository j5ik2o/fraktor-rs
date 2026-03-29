//! Graph-related abstractions.
//!
//! `GraphDslBuilder` provides a minimal builder surface over existing stream graphs.
//!
//! ```
//! use fraktor_stream_rs::core::{
//!   StreamNotUsed,
//!   graph::{FlowFragment, GraphDslBuilder},
//! };
//!
//! let _builder: GraphDslBuilder<u32, u32, StreamNotUsed> = GraphDslBuilder::new();
//! let _ = FlowFragment::<u32, u32, StreamNotUsed>::from_flow;
//! ```

// Bridge imports from core level for children
use super::{
  DynValue, SinkDecision, StageDefinition, StreamError, StreamPlan, SupervisionStrategy,
  buffer::{DemandTracker, StreamBuffer, StreamBufferConfig},
  lifecycle::{DriveOutcome, StreamState},
  mat::MatCombine,
  restart::RestartBackoff,
  shape,
  stage::{StageContext, StageKind},
};

mod flow_fragment;
mod graph_chain_macro;
mod graph_dsl;
mod graph_dsl_builder;
mod graph_interpreter;
mod graph_stage;
mod graph_stage_flow_adapter;
mod graph_stage_flow_context;
mod graph_stage_logic;
mod port_ops;
mod reverse_port_ops;
mod stream_graph;

pub use flow_fragment::FlowFragment;
pub use graph_dsl::GraphDsl;
pub use graph_dsl_builder::GraphDslBuilder;
pub use graph_interpreter::GraphInterpreter;
pub use graph_stage::GraphStage;
pub(crate) use graph_stage_flow_adapter::GraphStageFlowAdapter;
pub use graph_stage_logic::GraphStageLogic;
pub use port_ops::PortOps;
pub use reverse_port_ops::ReversePortOps;
pub use stream_graph::StreamGraph;
