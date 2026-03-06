//! Stage definitions for source, flow, and sink.

// Bridge submodules from core level
// Bridge types from core level for children
use super::{
  BoundedSourceQueue, DemandTracker, DynValue, FlowDefinition, FlowLogic, MatCombine, MatCombineRule, OverflowStrategy,
  RestartBackoff, RestartSettings, SinkDecision, SinkDefinition, SinkLogic, SourceDefinition, SourceLogic, SourceQueue,
  SourceQueueWithComplete, StageDefinition, StreamBufferConfig, StreamCompletion, StreamDone, StreamDslError,
  StreamError, StreamNotUsed, SupervisionStrategy, ThrottleMode, downcast_value, graph,
  graph::StreamGraph,
  keep_left, keep_right,
  lifecycle::{self, DriveOutcome},
  mat::{Materialized, Materializer, RunnableGraph},
  shape, validate_positive_argument,
};

/// Actor sink factory utilities.
mod actor_sink;
/// Async callback queue for stage logic.
mod async_callback;
/// Bidirectional flow definition.
mod bidi_flow;
/// Flow stage definitions.
pub mod flow;
/// Flow monitor handle.
mod flow_monitor;
/// Flow-oriented substream surface.
mod flow_sub_flow;
/// Context-preserving flow wrapper.
mod flow_with_context;
/// Restart DSL facade for flow stages.
mod restart_flow;
/// Restart DSL facade for sink stages.
mod restart_sink;
/// Restart DSL facade for source stages.
mod restart_source;
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
/// Timer helper for stage logic.
mod timer_graph_stage_logic;

// Internal re-exports for graph_interpreter tests
pub use actor_sink::ActorSink;
pub use async_callback::AsyncCallback;
pub use bidi_flow::BidiFlow;
pub use flow_monitor::FlowMonitor;
pub use flow_sub_flow::FlowSubFlow;
pub use flow_with_context::FlowWithContext;
pub use restart_flow::RestartFlow;
pub use restart_sink::RestartSink;
pub use restart_source::RestartSource;
pub use sink::Sink;
pub use source::Source;
pub use source_sub_flow::SourceSubFlow;
pub use source_with_context::SourceWithContext;
pub use stage_context::StageContext;
pub use stage_kind::StageKind;
pub use stream_stage::StreamStage;
pub use timer_graph_stage_logic::TimerGraphStageLogic;
