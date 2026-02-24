//! Stage definitions for source, flow, and sink.

// Bridge submodules from core level
// Bridge types from core level for children
use super::{
  DemandTracker, DynValue, FlowDefinition, FlowLogic, MatCombine, MatCombineRule, RestartBackoff, RestartSettings,
  SinkDecision, SinkDefinition, SinkLogic, SourceDefinition, SourceLogic, StageDefinition, StreamBufferConfig,
  StreamCompletion, StreamDone, StreamDslError, StreamError, StreamNotUsed, SupervisionStrategy, ThrottleMode,
  downcast_value, graph,
  graph::StreamGraph,
  keep_left, keep_right,
  lifecycle::{self, DriveOutcome},
  mat::{Materialized, Materializer, RunnableGraph},
  shape, validate_positive_argument,
};

/// Bidirectional flow definition.
mod bidi_flow;
/// Flow stage definitions.
mod flow;
/// Flow monitor handle.
mod flow_monitor;
/// Flow-oriented substream surface.
mod flow_sub_flow;
/// Context-preserving flow wrapper.
mod flow_with_context;
/// Sink stage definitions.
mod sink;
/// Source stage definitions.
mod source;
/// Source-oriented substream surface.
mod source_sub_flow;
/// Context-preserving source wrapper.
mod source_with_context;
/// Stage execution context.
mod stage_context;
/// Built-in stage kinds.
mod stage_kind;
/// Stream stage trait.
mod stream_stage;

// Internal re-exports for graph_interpreter tests
pub use bidi_flow::BidiFlow;
pub use flow::Flow;
#[allow(unused_imports)]
pub(in crate::core) use flow::{
  async_boundary_definition, balance_definition, broadcast_definition, buffer_definition, concat_definition,
  flat_map_merge_definition, interleave_definition, merge_definition, merge_latest_definition,
  merge_substreams_with_parallelism_definition, partition_definition, prepend_definition, split_after_definition,
  split_when_definition, unzip_definition, unzip_with_definition, zip_all_definition, zip_definition,
};
pub use flow_monitor::FlowMonitor;
pub use flow_sub_flow::FlowSubFlow;
pub use flow_with_context::FlowWithContext;
pub use sink::Sink;
pub use source::Source;
pub use source_sub_flow::SourceSubFlow;
pub use source_with_context::SourceWithContext;
pub use stage_context::StageContext;
pub use stage_kind::StageKind;
pub use stream_stage::StreamStage;
