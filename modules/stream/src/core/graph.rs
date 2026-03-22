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
  DemandTracker, DynValue, MatCombine, RestartBackoff, SinkDecision, StageDefinition, StreamBuffer, StreamBufferConfig,
  StreamError, StreamPlan, SupervisionStrategy,
  lifecycle::{DriveOutcome, StreamState},
  shape,
  stage::{StageContext, StageKind},
};

mod flow_fragment;
mod graph_dsl;
mod graph_dsl_builder;
mod graph_interpreter;
mod graph_stage;
mod graph_stage_logic;
mod stream_graph;

pub use flow_fragment::FlowFragment;
pub use graph_dsl::GraphDsl;
pub use graph_dsl_builder::GraphDslBuilder;
pub use graph_interpreter::GraphInterpreter;
pub use graph_stage::GraphStage;
pub use graph_stage_logic::GraphStageLogic;
pub use stream_graph::StreamGraph;
