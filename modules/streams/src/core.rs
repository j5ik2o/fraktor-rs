/// Actor-backed materializer implementation.
mod actor_materializer;
/// Actor materializer configuration.
mod actor_materializer_config;
/// Bidirectional flow definition.
mod bidi_flow;
/// Bidirectional shape definition.
mod bidi_shape;
/// Completion polling types.
mod completion;
/// Demand model types.
mod demand;
/// Demand tracking utilities.
mod demand_tracker;
/// Drive outcome enums.
mod drive_outcome;
/// Flow stage definitions.
mod flow;
/// Flow shape definition.
mod flow_shape;
/// GraphDSL-like partial graph builder.
mod graph_dsl;
/// Graph interpreter runtime.
mod graph_interpreter;
/// Graph stage abstractions.
mod graph_stage;
/// Graph stage logic abstractions.
mod graph_stage_logic;
/// Typed inlet ports.
mod inlet;
/// Keep-both materialization rule.
mod keep_both;
/// Keep-left materialization rule.
mod keep_left;
/// Keep-none materialization rule.
mod keep_none;
/// Keep-right materialization rule.
mod keep_right;
/// Materialization combination kinds.
mod mat_combine;
/// Materialization combination rules.
mod mat_combine_rule;
/// Materialized result wrapper.
mod materialized;
/// Materializer trait.
mod materializer;
/// Typed outlet ports.
mod outlet;
/// Port identifier type.
mod port_id;
/// Runnable graph type.
mod runnable_graph;
/// Shape abstraction.
mod shape;
/// Sink stage definitions.
mod sink;
/// Sink shape definition.
mod sink_shape;
/// Source stage definitions.
mod source;
/// Source shape definition.
mod source_shape;
/// Stage execution context.
mod stage_context;
/// Built-in stage kinds.
mod stage_kind;
/// Stream execution state (internal).
mod stream;
/// Stream buffer implementation.
mod stream_buffer;
/// Stream buffer configuration.
mod stream_buffer_config;
/// Stream completion handle.
mod stream_completion;
/// Stream completion marker.
mod stream_done;
/// Stream drive actor (internal).
mod stream_drive_actor;
/// Stream drive command (internal).
mod stream_drive_command;
/// Stream error definitions.
mod stream_error;
/// Stream graph structure.
mod stream_graph;
/// Stream handle trait.
mod stream_handle;
/// Stream handle implementation.
mod stream_handle_generic;
/// Stream handle identifier.
mod stream_handle_id;
/// Stream not-used marker.
mod stream_not_used;
/// Stream shape definition.
mod stream_shape;
/// Stream shared wrapper (internal).
mod stream_shared;
/// Stream stage trait.
mod stream_stage;
/// Stream state enum.
mod stream_state;

use alloc::{boxed::Box, vec::Vec};
use core::any::{Any, TypeId};

pub use actor_materializer::ActorMaterializerGeneric;
pub use actor_materializer_config::ActorMaterializerConfig;
pub use bidi_flow::BidiFlow;
pub use bidi_shape::BidiShape;
pub use completion::Completion;
pub use demand::Demand;
pub use demand_tracker::DemandTracker;
pub use drive_outcome::DriveOutcome;
pub use flow::Flow;
pub use flow_shape::FlowShape;
pub use graph_dsl::GraphDsl;
pub use graph_interpreter::GraphInterpreter;
pub use graph_stage::GraphStage;
pub use graph_stage_logic::GraphStageLogic;
pub use inlet::Inlet;
pub use keep_both::KeepBoth;
pub use keep_left::KeepLeft;
pub use keep_none::KeepNone;
pub use keep_right::KeepRight;
pub use mat_combine::MatCombine;
pub use mat_combine_rule::MatCombineRule;
pub use materialized::Materialized;
pub use materializer::Materializer;
pub use outlet::Outlet;
pub use port_id::PortId;
pub use runnable_graph::RunnableGraph;
pub use shape::Shape;
pub use sink::Sink;
pub use sink_shape::SinkShape;
pub use source::Source;
pub use source_shape::SourceShape;
pub use stage_context::StageContext;
pub use stage_kind::StageKind;
pub use stream_buffer::StreamBuffer;
pub use stream_buffer_config::StreamBufferConfig;
pub use stream_completion::StreamCompletion;
pub use stream_done::StreamDone;
pub use stream_error::StreamError;
pub use stream_graph::StreamGraph;
pub use stream_handle::StreamHandle;
pub use stream_handle_generic::StreamHandleGeneric;
pub use stream_handle_id::StreamHandleId;
pub use stream_not_used::StreamNotUsed;
pub use stream_shape::StreamShape;
pub use stream_stage::StreamStage;
pub use stream_state::StreamState;
type DynValue = Box<dyn Any + Send + Sync + 'static>;

enum StageDefinition {
  Source(SourceDefinition),
  Flow(FlowDefinition),
  Sink(SinkDefinition),
}

impl StageDefinition {
  const fn inlet(&self) -> Option<PortId> {
    match self {
      | Self::Source(_) => None,
      | Self::Flow(definition) => Some(definition.inlet),
      | Self::Sink(definition) => Some(definition.inlet),
    }
  }

  const fn outlet(&self) -> Option<PortId> {
    match self {
      | Self::Source(definition) => Some(definition.outlet),
      | Self::Flow(definition) => Some(definition.outlet),
      | Self::Sink(_) => None,
    }
  }

  const fn kind(&self) -> StageKind {
    match self {
      | Self::Source(definition) => definition.kind,
      | Self::Flow(definition) => definition.kind,
      | Self::Sink(definition) => definition.kind,
    }
  }

  const fn mat_combine(&self) -> MatCombine {
    match self {
      | Self::Source(definition) => definition.mat_combine,
      | Self::Flow(definition) => definition.mat_combine,
      | Self::Sink(definition) => definition.mat_combine,
    }
  }
}

struct SourceDefinition {
  kind:        StageKind,
  outlet:      PortId,
  output_type: TypeId,
  mat_combine: MatCombine,
  logic:       Box<dyn SourceLogic>,
}

struct FlowDefinition {
  kind:        StageKind,
  inlet:       PortId,
  outlet:      PortId,
  input_type:  TypeId,
  output_type: TypeId,
  mat_combine: MatCombine,
  logic:       Box<dyn FlowLogic>,
}

struct SinkDefinition {
  kind:        StageKind,
  inlet:       PortId,
  input_type:  TypeId,
  mat_combine: MatCombine,
  logic:       Box<dyn SinkLogic>,
}

/// Materialization-ready immutable blueprint.
///
/// The plan contains only stage definitions and wiring edges.
/// Mutable execution state is created by the interpreter during materialization.
struct StreamPlan {
  stages: Vec<StageDefinition>,
  edges:  Vec<(PortId, PortId, MatCombine)>,
}

impl StreamPlan {
  const fn from_parts(stages: Vec<StageDefinition>, edges: Vec<(PortId, PortId, MatCombine)>) -> Self {
    Self { stages, edges }
  }
}

trait SourceLogic: Send {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError>;
}

trait FlowLogic: Send {
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError>;

  fn apply_with_edge(&mut self, edge_index: usize, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let _ = edge_index;
    self.apply(input)
  }

  fn expected_fan_out(&self) -> Option<usize> {
    None
  }

  fn expected_fan_in(&self) -> Option<usize> {
    None
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    Ok(())
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    Ok(Vec::new())
  }
}

enum SinkDecision {
  Continue,
  Complete,
}

trait SinkLogic: Send {
  fn on_start(&mut self, demand: &mut DemandTracker) -> Result<(), StreamError>;
  fn on_push(&mut self, input: DynValue, demand: &mut DemandTracker) -> Result<SinkDecision, StreamError>;
  fn on_complete(&mut self) -> Result<(), StreamError>;
  fn on_error(&mut self, error: StreamError);
}

fn downcast_value<In>(value: DynValue) -> Result<In, StreamError>
where
  In: Any + Send + Sync + 'static, {
  match value.downcast::<In>() {
    | Ok(value) => Ok(*value),
    | Err(_) => Err(StreamError::TypeMismatch),
  }
}
