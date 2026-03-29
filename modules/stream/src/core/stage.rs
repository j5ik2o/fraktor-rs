//! GraphStage primitives and stage helpers.
//!
//! ```compile_fail
//! use fraktor_stream_rs::core::stage::{flow::Flow, sink::Sink, source::Source};
//! ```

use alloc::{boxed::Box, vec::Vec};

// Bridge submodules from core level
// Bridge types from core level for children
use super::{
  DynValue, FlowDefinition, FlowLogic, SinkDecision, SinkDefinition, SinkLogic, SourceDefinition, SourceLogic,
  StageDefinition, StreamDone, StreamDslError, StreamError, StreamNotUsed, SupervisionStrategy, ThrottleMode,
  buffer::{DemandTracker, OverflowStrategy, StreamBufferConfig},
  downcast_value, graph,
  graph::StreamGraph,
  lifecycle::{self, DriveOutcome},
  mat::MatCombine,
  materialization::{KeepLeft, KeepRight, MatCombineRule, Materialized, Materializer, RunnableGraph, StreamCompletion},
  queue::{BoundedSourceQueue, SourceQueue, SourceQueueWithComplete},
  restart::{RestartBackoff, RestartSettings},
  shape,
  validate_positive_argument::validate_positive_argument,
};

/// Actor sink factory utilities.
mod actor_sink;
/// Actor source factory utilities.
mod actor_source;
/// Async callback queue for stage logic.
mod async_callback;
/// Bidirectional flow definition.
mod bidi_flow;
/// Flow stage definitions.
mod flow;
/// `group_by`-specific substream surface for flows.
mod flow_group_by_sub_flow;
/// Flow monitor handle.
mod flow_monitor;
/// Default flow monitor implementation.
mod flow_monitor_impl;
/// Stream state tracked by a flow monitor.
mod flow_monitor_state;
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
/// `group_by`-specific substream surface for sources.
mod source_group_by_sub_flow;
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
/// Lazy tail source wrapper.
mod tail_source;
/// Timer helper for stage logic.
mod timer_graph_stage_logic;
/// Topic-based pub/sub stream integration.
mod topic_pub_sub;

pub use async_callback::AsyncCallback;
pub use flow_monitor::FlowMonitor;
pub use flow_monitor_state::FlowMonitorState;
pub use stage_context::StageContext;
pub use stage_kind::StageKind;
pub use stream_stage::StreamStage;
pub use timer_graph_stage_logic::TimerGraphStageLogic;

pub(in crate::core) type ActorSink = actor_sink::ActorSink;
pub(in crate::core) type ActorSource = actor_source::ActorSource;
pub(in crate::core) type BidiFlow<InTop, OutTop, InBottom, OutBottom, Mat> =
  bidi_flow::BidiFlow<InTop, OutTop, InBottom, OutBottom, Mat>;
pub(in crate::core) type Flow<In, Out, Mat> = flow::Flow<In, Out, Mat>;
pub(in crate::core) type FlowGroupBySubFlow<In, Key, Out, Mat> =
  flow_group_by_sub_flow::FlowGroupBySubFlow<In, Key, Out, Mat>;
pub(in crate::core) type FlowMonitorImpl<Out> = flow_monitor_impl::FlowMonitorImpl<Out>;
pub(in crate::core) type FlowSubFlow<In, Out, Mat> = flow_sub_flow::FlowSubFlow<In, Out, Mat>;
pub(in crate::core) type FlowWithContext<Ctx, In, Out, Mat> = flow_with_context::FlowWithContext<Ctx, In, Out, Mat>;
pub(in crate::core) type RestartFlow = restart_flow::RestartFlow;
pub(in crate::core) type RestartSink = restart_sink::RestartSink;
pub(in crate::core) type RestartSource = restart_source::RestartSource;
pub(in crate::core) type Sink<In, Mat> = sink::Sink<In, Mat>;
pub(in crate::core) type Source<Out, Mat> = source::Source<Out, Mat>;
pub(in crate::core) type SourceGroupBySubFlow<Key, Out, Mat> =
  source_group_by_sub_flow::SourceGroupBySubFlow<Key, Out, Mat>;
pub(in crate::core) type SourceSubFlow<Out, Mat> = source_sub_flow::SourceSubFlow<Out, Mat>;
pub(in crate::core) type SourceWithContext<Ctx, Out, Mat> = source_with_context::SourceWithContext<Ctx, Out, Mat>;
pub(in crate::core) type TailSource<Out> = tail_source::TailSource<Out>;
pub(in crate::core) type TopicPubSub = topic_pub_sub::TopicPubSub;

pub(in crate::core) fn combine_mat<Left, Right, C>(left: Left, right: Right) -> C::Out
where
  C: MatCombineRule<Left, Right>, {
  flow::combine_mat::<Left, Right, C>(left, right)
}

pub(in crate::core) fn retry_flow_definition<In, Out, R>(
  inner_logics: Vec<Box<dyn FlowLogic>>,
  decide_retry: R,
  max_retries: usize,
  min_backoff_ticks: u32,
  max_backoff_ticks: u32,
  random_factor_permille: u16,
) -> FlowDefinition
where
  In: Clone + Send + Sync + 'static,
  Out: Send + Sync + 'static,
  R: Fn(&In, &Out) -> Option<In> + Send + 'static, {
  flow::retry_flow_definition::<In, Out, R>(
    inner_logics,
    decide_retry,
    max_retries,
    min_backoff_ticks,
    max_backoff_ticks,
    random_factor_permille,
  )
}

/// Extracts the last context and collects values from a context-value pair sequence.
///
/// Used by `FlowWithContext` and `SourceWithContext` for `grouped` / `sliding`.
pub(crate) fn extract_last_ctx_and_values<Ctx, V>(pairs: Vec<(Ctx, V)>) -> Option<(Ctx, Vec<V>)> {
  let mut last_ctx = None;
  let values: Vec<V> = pairs
    .into_iter()
    .map(|(ctx, v)| {
      last_ctx = Some(ctx);
      v
    })
    .collect();
  last_ctx.map(|ctx| (ctx, values))
}
