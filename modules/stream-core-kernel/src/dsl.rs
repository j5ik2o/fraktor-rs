//! Public stream DSL surface.
//!
//! This module gathers the leaf types used to build stream graphs from sources,
//! flows, and sinks without exposing the internal package layout.
//!
//! ```compile_fail
//! use fraktor_stream_core_kernel_rs::{
//!   StreamNotUsed,
//!   stage::{Flow, Sink, Source},
//! };
//!
//! let _ = Flow::<u32, u32, StreamNotUsed>::new();
//! let _ = Source::<u32, StreamNotUsed>::single(1);
//! let _ = Sink::<u32, StreamNotUsed>::ignore();
//! ```

// Bridge imports from core level for children
use super::{
  BoundedSourceQueue, DynValue, FlowDefinition, FlowLogic, OverflowStrategy, RestartConfig, SinkDecision,
  SinkDefinition, SinkLogic, SourceDefinition, SourceLogic, StageDefinition, StreamDslError, StreamError,
  SupervisionStrategy, ThrottleMode, downcast_value,
  r#impl::{
    RestartBackoff, StreamGraph,
    fusing::{DemandTracker, StreamBufferConfig},
  },
  materialization::{
    KeepLeft, KeepRight, MatCombine, MatCombineRule, Materialized, Materializer, RunnableGraph, StreamDone,
    StreamFuture, StreamNotUsed,
  },
  shape,
  stage::{StageContext, StageKind, extract_last_ctx_and_values},
  validate_positive_argument,
};

mod actor_sink;
mod actor_source;
mod bidi_flow;
mod broadcast_hub;
#[cfg(feature = "compression")]
mod compression;
mod coupled_termination_flow;
mod delay_strategy;
mod draining_control;
mod fixed_delay;
mod flow;
mod flow_group_by_sub_flow;
mod flow_monitor;
mod flow_monitor_impl;
mod flow_monitor_state;
mod flow_sub_flow;
mod flow_with_context;
mod framing;
mod graph_dsl;
mod graph_dsl_builder;
mod hub;
mod json_framing;
mod linear_increasing_delay;
mod merge_hub;
mod partition_hub;
mod queue;
mod restart_flow;
mod restart_sink;
mod restart_source;
mod retry_flow;
mod sink;
mod sink_queue;
mod source;
mod source_group_by_sub_flow;
mod source_queue;
mod source_queue_with_complete;
mod source_sub_flow;
mod source_with_context;
mod stateful_map_concat_accumulator;
mod stream_refs;
mod tail_source;
#[cfg(test)]
mod tests;

pub use actor_sink::ActorSink;
pub use actor_source::ActorSource;
pub use bidi_flow::BidiFlow;
pub use broadcast_hub::BroadcastHub;
#[cfg(feature = "compression")]
pub use compression::Compression;
pub use coupled_termination_flow::CoupledTerminationFlow;
pub use delay_strategy::DelayStrategy;
pub use draining_control::DrainingControl;
pub use fixed_delay::FixedDelay;
pub use flow::Flow;
pub(crate) use flow::{combine_mat, retry_flow_definition};
pub use flow_group_by_sub_flow::FlowGroupBySubFlow;
pub use flow_monitor::FlowMonitor;
pub use flow_monitor_impl::FlowMonitorImpl;
pub use flow_monitor_state::FlowMonitorState;
pub use flow_sub_flow::FlowSubFlow;
pub use flow_with_context::FlowWithContext;
pub use framing::Framing;
pub use graph_dsl::GraphDsl;
pub use graph_dsl_builder::GraphDslBuilder;
pub use json_framing::JsonFraming;
pub use linear_increasing_delay::LinearIncreasingDelay;
pub use merge_hub::MergeHub;
pub use partition_hub::PartitionHub;
pub use restart_flow::RestartFlow;
pub use restart_sink::RestartSink;
pub use restart_source::RestartSource;
pub use retry_flow::RetryFlow;
pub use sink::Sink;
pub use sink_queue::{SinkQueue, SinkQueueWithCancel};
pub use source::Source;
pub use source_group_by_sub_flow::SourceGroupBySubFlow;
pub use source_queue::SourceQueue;
pub use source_queue_with_complete::SourceQueueWithComplete;
pub use source_sub_flow::SourceSubFlow;
pub use source_with_context::SourceWithContext;
pub use stateful_map_concat_accumulator::StatefulMapConcatAccumulator;
pub use stream_refs::StreamRefs;
pub use tail_source::TailSource;
