//! Graph-related abstractions.
//!
//! Reusable flow fragments intentionally exclude `GraphDSL.Builder`-style arbitrary port wiring.
//!
//! ```compile_fail
//! use fraktor_streams_rs::core::graph::{FlowFragment, GraphDslBuilder};
//!
//! let _builder: GraphDslBuilder<u32, u32, ()>;
//! let _ = FlowFragment::from_flow;
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
mod graph_interpreter;
mod graph_stage;
mod graph_stage_logic;
mod stream_graph;

pub use flow_fragment::FlowFragment;
pub use graph_interpreter::GraphInterpreter;
pub use graph_stage::GraphStage;
pub use graph_stage_logic::GraphStageLogic;
pub use stream_graph::StreamGraph;
