#![deny(missing_docs)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::disallowed_types, clippy::redundant_clone))]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::missing_errors_doc)]
#![deny(clippy::missing_panics_doc)]
#![deny(clippy::missing_safety_doc)]
#![cfg_attr(not(test), deny(clippy::redundant_clone))]
#![deny(clippy::redundant_field_names)]
#![deny(clippy::redundant_pattern)]
#![deny(clippy::redundant_static_lifetimes)]
#![deny(clippy::unnecessary_to_owned)]
#![deny(clippy::unnecessary_struct_initialization)]
#![deny(clippy::needless_borrow)]
#![deny(clippy::needless_pass_by_value)]
#![deny(clippy::manual_ok_or)]
#![deny(clippy::manual_map)]
#![deny(clippy::manual_let_else)]
#![deny(clippy::manual_strip)]
#![deny(clippy::unused_async)]
#![deny(clippy::unused_self)]
#![deny(clippy::unnecessary_wraps)]
#![deny(clippy::unreachable)]
#![deny(clippy::empty_enums)]
#![deny(clippy::no_effect)]
#![deny(dropping_copy_types)]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]
#![cfg_attr(not(test), deny(clippy::expect_used))]
#![deny(clippy::todo)]
#![deny(clippy::unimplemented)]
#![deny(clippy::print_stdout)]
#![deny(clippy::dbg_macro)]
#![deny(clippy::missing_const_for_fn)]
#![deny(clippy::must_use_candidate)]
#![deny(clippy::trivially_copy_pass_by_ref)]
#![deny(clippy::clone_on_copy)]
#![deny(clippy::len_without_is_empty)]
#![deny(clippy::wrong_self_convention)]
#![deny(clippy::from_over_into)]
#![deny(clippy::eq_op)]
#![deny(clippy::bool_comparison)]
#![deny(clippy::needless_bool)]
#![deny(clippy::match_like_matches_macro)]
#![deny(clippy::manual_assert)]
#![deny(clippy::naive_bytecount)]
#![deny(clippy::if_same_then_else)]
#![deny(clippy::cmp_null)]
#![deny(unreachable_pub)]
#![allow(unknown_lints)]
#![deny(cfg_std_forbid)]
#![cfg_attr(not(test), no_std)]

//! Stream processing primitives for fraktor runtime.

extern crate alloc;

/// Stream attributes for stage and graph metadata.
pub mod attributes;
/// Bounded queue materialized by `Source::queue`.
mod bounded_source_queue;
/// Completion strategy for stream termination.
mod completion_strategy;
/// Public stream DSL surface.
pub mod dsl;
/// Internal implementation packages mirroring Pekko's `impl` boundary.
pub mod r#impl;
/// Kill switch shared contract trait.
mod kill_switch;
/// Kill switch factory functions.
mod kill_switches;
/// Overflow strategy for bounded queues.
mod overflow_strategy;
/// Decision returned from sink logic callbacks.
mod sink_decision;
/// Sink-stage callback contract.
mod sink_logic;
// framing moved to dsl/framing
/// IO operation result type.
mod io_result;
// json_framing moved to dsl/json_framing
/// Materialization contracts and lifecycle types.
pub mod materialization;
/// Result of offering an element into a source queue.
mod queue_offer_result;
/// Restart and backoff configuration.
mod restart_config;
/// Restart log configuration.
mod restart_log_config;
/// Restart log level enum.
mod restart_log_level;
/// Stream topology shapes and connection points.
pub mod shape;
/// Shared kill switch for multi-stream control.
mod shared_kill_switch;
/// Materializer state snapshot support.
pub mod snapshot;
/// Source-stage callback contract.
mod source_logic;
/// Stage definitions for source, flow, and sink.
pub mod stage;
/// Stream reference public contracts.
pub mod stream_ref;
/// Unique kill switch for single-stream control.
mod unique_kill_switch;
// stateful_map_concat_accumulator moved to dsl/stateful_map_concat_accumulator
/// Termination mode for stream subscription timeout.
mod stream_subscription_timeout_termination_mode;
/// `split_when` / `split_after` substream cancellation strategy.
mod substream_cancel_strategy;
/// Supervision strategy definitions.
mod supervision_strategy;
/// Test utilities for stream verification.
pub mod testing;
/// Throttle behavior mode.
mod throttle_mode;

use alloc::{boxed::Box, vec::Vec};
use core::any::{Any, TypeId};

pub use bounded_source_queue::BoundedSourceQueue;
pub use completion_strategy::CompletionStrategy;
#[cfg(feature = "compression")]
pub use dsl::Compression;
use fraktor_actor_core_kernel_rs::system::ActorSystem;
use fraktor_utils_core_rs::sync::ArcShared;
use r#impl::{
  RestartBackoff, StreamDslError as ImplStreamDslError, StreamError as ImplStreamError,
  fusing::DemandTracker as InternalDemandTracker, validate_positive_argument as internal_validate_positive_argument,
};
pub use io_result::IOResult;
pub use kill_switch::KillSwitch;
pub use kill_switches::KillSwitches;
use materialization::MatCombine;
pub use overflow_strategy::OverflowStrategy;
pub use queue_offer_result::QueueOfferResult;
pub use restart_config::RestartConfig;
pub use restart_log_config::RestartLogConfig;
pub use restart_log_level::RestartLogLevel;
use shape::PortId;
pub use shared_kill_switch::SharedKillSwitch;
pub use sink_decision::SinkDecision;
pub use sink_logic::SinkLogic;
pub use source_logic::SourceLogic;
use stage::StageKind;
/// Tracks downstream demand for sink stages.
pub type DemandTracker = InternalDemandTracker;
/// Public alias for stream DSL construction errors.
pub type StreamDslError = ImplStreamDslError;
/// Public alias for stream execution errors.
pub type StreamError = ImplStreamError;
// StatefulMapConcatAccumulator re-exported via dsl::StatefulMapConcatAccumulator
pub use stream_subscription_timeout_termination_mode::StreamSubscriptionTimeoutTerminationMode;
pub use substream_cancel_strategy::SubstreamCancelStrategy;
pub use supervision_strategy::SupervisionStrategy;
pub use throttle_mode::ThrottleMode;
pub use unique_kill_switch::UniqueKillSwitch;
pub(crate) use unique_kill_switch::{
  KillSwitchCommandTarget, KillSwitchCommandTargetShared, KillSwitchState, KillSwitchStateHandle, KillSwitchStatus,
};

use self::attributes::Attributes;
/// Type-erased value passed between runtime stages.
pub type DynValue = Box<dyn Any + Send + 'static>;

/// Validates that the provided argument is greater than zero.
///
/// # Errors
///
/// Returns [`StreamDslError::InvalidArgument`] when `value == 0`.
pub const fn validate_positive_argument(name: &'static str, value: usize) -> Result<usize, StreamDslError> {
  internal_validate_positive_argument(name, value)
}

pub(crate) enum StageDefinition {
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

  /// Returns the output `TypeId` for stages that have an outlet.
  const fn output_type(&self) -> Option<TypeId> {
    match self {
      | Self::Source(definition) => Some(definition.output_type),
      | Self::Flow(definition) => Some(definition.output_type),
      | Self::Sink(_) => None,
    }
  }

  /// Returns the per-stage attributes.
  pub(crate) const fn attributes(&self) -> &Attributes {
    match self {
      | Self::Source(definition) => &definition.attributes,
      | Self::Flow(definition) => &definition.attributes,
      | Self::Sink(definition) => &definition.attributes,
    }
  }

  /// Returns a new stage definition with the given attributes merged.
  fn with_attributes(self, attrs: Attributes) -> Self {
    match self {
      | Self::Source(mut definition) => {
        let old = core::mem::take(&mut definition.attributes);
        definition.attributes = old.and(attrs);
        Self::Source(definition)
      },
      | Self::Flow(mut definition) => {
        let old = core::mem::take(&mut definition.attributes);
        definition.attributes = old.and(attrs);
        Self::Flow(definition)
      },
      | Self::Sink(mut definition) => {
        let old = core::mem::take(&mut definition.attributes);
        definition.attributes = old.and(attrs);
        Self::Sink(definition)
      },
    }
  }
}

pub(crate) struct SourceDefinition {
  pub(crate) kind:        StageKind,
  pub(crate) outlet:      PortId,
  pub(crate) output_type: TypeId,
  pub(crate) mat_combine: MatCombine,
  pub(crate) supervision: SupervisionStrategy,
  pub(crate) restart:     Option<RestartBackoff>,
  pub(crate) logic:       Box<dyn SourceLogic>,
  pub(crate) attributes:  Attributes,
}

pub(crate) struct FlowDefinition {
  pub(crate) kind:        StageKind,
  pub(crate) inlet:       PortId,
  pub(crate) outlet:      PortId,
  pub(crate) input_type:  TypeId,
  pub(crate) output_type: TypeId,
  pub(crate) mat_combine: MatCombine,
  pub(crate) supervision: SupervisionStrategy,
  pub(crate) restart:     Option<RestartBackoff>,
  pub(crate) logic:       Box<dyn FlowLogic>,
  pub(crate) attributes:  Attributes,
}

pub(crate) struct SinkDefinition {
  pub(crate) kind:        StageKind,
  pub(crate) inlet:       PortId,
  pub(crate) input_type:  TypeId,
  pub(crate) mat_combine: MatCombine,
  pub(crate) supervision: SupervisionStrategy,
  pub(crate) restart:     Option<RestartBackoff>,
  pub(crate) logic:       Box<dyn SinkLogic>,
  pub(crate) attributes:  Attributes,
}

/// Materialization-ready immutable blueprint.
///
/// The plan contains only stage definitions and wiring edges.
/// Mutable execution state is created by the interpreter during materialization.
pub(crate) struct StreamPlan {
  pub(crate) stages:         Vec<StageDefinition>,
  pub(crate) edges:          Vec<StreamPlanEdge>,
  pub(crate) source_indices: Vec<usize>,
  pub(crate) sink_indices:   Vec<usize>,
  pub(crate) flow_order:     Vec<usize>,
  kill_switch_states:        Vec<KillSwitchStateHandle>,
}

struct StreamPlanStagePorts {
  source_indices: Vec<usize>,
  sink_indices:   Vec<usize>,
  input_ports:    Vec<(PortId, usize)>,
  output_ports:   Vec<(PortId, usize)>,
}

struct StreamPlanTopology {
  edges:     Vec<StreamPlanEdge>,
  incoming:  Vec<usize>,
  outgoing:  Vec<usize>,
  adjacency: Vec<Vec<usize>>,
}

impl StreamPlan {
  pub(crate) fn from_parts(
    stages: Vec<StageDefinition>,
    edges: Vec<(PortId, PortId, MatCombine)>,
  ) -> Result<Self, StreamError> {
    if stages.is_empty() || edges.is_empty() {
      return Err(StreamError::InvalidConnection);
    }

    let stage_ports = collect_stream_plan_stage_ports(&stages)?;
    validate_stream_plan_stage_presence(&stage_ports)?;
    let topology = build_stream_plan_topology(stages.len(), edges, &stage_ports)?;
    validate_stream_plan_degrees(&stages, &topology)?;
    let ordered_indices = topological_stream_plan_order(stages.len(), topology.incoming, &topology.adjacency)?;
    let flow_order = stream_plan_flow_order(&stages, ordered_indices);

    Ok(Self {
      stages,
      edges: topology.edges,
      source_indices: stage_ports.source_indices,
      sink_indices: stage_ports.sink_indices,
      flow_order,
      kill_switch_states: Vec::new(),
    })
  }

  /// Creates a `StreamPlan` from pre-validated parts (used by island splitting).
  ///
  /// The caller is responsible for ensuring that stages, edges, and indices
  /// are consistent. No validation is performed.
  pub(crate) const fn from_raw_parts(
    stages: Vec<StageDefinition>,
    edges: Vec<StreamPlanEdge>,
    source_indices: Vec<usize>,
    sink_indices: Vec<usize>,
    flow_order: Vec<usize>,
    kill_switch_states: Vec<KillSwitchStateHandle>,
  ) -> Self {
    Self { stages, edges, source_indices, sink_indices, flow_order, kill_switch_states }
  }

  fn with_shared_kill_switch_state(mut self, kill_switch_state: KillSwitchStateHandle) -> Self {
    if self.kill_switch_states.iter().any(|existing| ArcShared::ptr_eq(existing, &kill_switch_state)) {
      return self;
    }
    self.kill_switch_states.push(kill_switch_state);
    self
  }

  fn shared_kill_switch_states(&self) -> &[KillSwitchStateHandle] {
    &self.kill_switch_states
  }
}

fn collect_stream_plan_stage_ports(stages: &[StageDefinition]) -> Result<StreamPlanStagePorts, StreamError> {
  let mut source_indices = Vec::new();
  let mut sink_indices = Vec::new();
  let mut input_ports = Vec::with_capacity(stages.len());
  let mut output_ports = Vec::with_capacity(stages.len());

  for (stage_index, stage) in stages.iter().enumerate() {
    match stage {
      | StageDefinition::Source(_) => source_indices.push(stage_index),
      | StageDefinition::Flow(_) => {},
      | StageDefinition::Sink(_) => sink_indices.push(stage_index),
    }
    collect_unique_stage_port(&mut input_ports, stage.inlet(), stage_index)?;
    collect_unique_stage_port(&mut output_ports, stage.outlet(), stage_index)?;
  }

  Ok(StreamPlanStagePorts { source_indices, sink_indices, input_ports, output_ports })
}

fn collect_unique_stage_port(
  ports: &mut Vec<(PortId, usize)>,
  port: Option<PortId>,
  stage_index: usize,
) -> Result<(), StreamError> {
  let Some(port) = port else {
    return Ok(());
  };
  if ports.iter().any(|(existing, _)| *existing == port) {
    return Err(StreamError::InvalidConnection);
  }
  ports.push((port, stage_index));
  Ok(())
}

const fn validate_stream_plan_stage_presence(stage_ports: &StreamPlanStagePorts) -> Result<(), StreamError> {
  if stage_ports.source_indices.is_empty() || stage_ports.sink_indices.is_empty() {
    return Err(StreamError::InvalidConnection);
  }
  Ok(())
}

fn build_stream_plan_topology(
  stage_count: usize,
  edges: Vec<(PortId, PortId, MatCombine)>,
  stage_ports: &StreamPlanStagePorts,
) -> Result<StreamPlanTopology, StreamError> {
  let mut topology = empty_stream_plan_topology(stage_count, edges.len());
  for (from, to, mat) in edges {
    let from_stage = stage_index_for_port(&stage_ports.output_ports, from)?;
    let to_stage = stage_index_for_port(&stage_ports.input_ports, to)?;
    topology.outgoing[from_stage] += 1;
    topology.incoming[to_stage] += 1;
    topology.adjacency[from_stage].push(to_stage);
    topology.edges.push(StreamPlanEdge { from_port: from, to_port: to, mat });
  }
  Ok(topology)
}

fn empty_stream_plan_topology(stage_count: usize, edge_count: usize) -> StreamPlanTopology {
  StreamPlanTopology {
    edges:     Vec::with_capacity(edge_count),
    incoming:  alloc::vec![0_usize; stage_count],
    outgoing:  alloc::vec![0_usize; stage_count],
    adjacency: alloc::vec![Vec::new(); stage_count],
  }
}

fn stage_index_for_port(ports: &[(PortId, usize)], port: PortId) -> Result<usize, StreamError> {
  ports
    .iter()
    .find(|(existing, _)| *existing == port)
    .map(|(_, stage_index)| *stage_index)
    .ok_or(StreamError::InvalidConnection)
}

fn validate_stream_plan_degrees(stages: &[StageDefinition], topology: &StreamPlanTopology) -> Result<(), StreamError> {
  for (stage_index, stage) in stages.iter().enumerate() {
    validate_stream_plan_stage_degree(stage, topology.incoming[stage_index], topology.outgoing[stage_index])?;
  }
  Ok(())
}

fn validate_stream_plan_stage_degree(
  stage: &StageDefinition,
  incoming: usize,
  outgoing: usize,
) -> Result<(), StreamError> {
  match stage {
    | StageDefinition::Source(_) => validate_source_degree(outgoing),
    | StageDefinition::Flow(definition) => validate_flow_degree(definition, incoming, outgoing),
    | StageDefinition::Sink(_) => validate_sink_degree(incoming),
  }
}

const fn validate_source_degree(outgoing: usize) -> Result<(), StreamError> {
  if outgoing == 0 {
    return Err(StreamError::InvalidConnection);
  }
  Ok(())
}

fn validate_flow_degree(definition: &FlowDefinition, incoming: usize, outgoing: usize) -> Result<(), StreamError> {
  if incoming == 0 || outgoing == 0 {
    return Err(StreamError::InvalidConnection);
  }
  if let Some(expected_fan_in) = definition.logic.expected_fan_in()
    && incoming != expected_fan_in
  {
    return Err(StreamError::InvalidConnection);
  }
  if let Some(expected_fan_out) = definition.logic.expected_fan_out()
    && outgoing != expected_fan_out
  {
    return Err(StreamError::InvalidConnection);
  }
  Ok(())
}

const fn validate_sink_degree(incoming: usize) -> Result<(), StreamError> {
  if incoming == 0 {
    return Err(StreamError::InvalidConnection);
  }
  Ok(())
}

fn topological_stream_plan_order(
  stage_count: usize,
  mut incoming: Vec<usize>,
  adjacency: &[Vec<usize>],
) -> Result<Vec<usize>, StreamError> {
  let mut ready = stream_plan_ready_stages(&incoming);
  let mut ordered_indices = Vec::new();

  while let Some(stage_index) = ready.pop() {
    ordered_indices.push(stage_index);
    collect_ready_downstream_stages(stage_index, adjacency, &mut incoming, &mut ready);
  }

  if ordered_indices.len() != stage_count {
    return Err(StreamError::InvalidConnection);
  }
  Ok(ordered_indices)
}

fn stream_plan_ready_stages(incoming: &[usize]) -> Vec<usize> {
  let mut ready = Vec::new();
  for (stage_index, count) in incoming.iter().enumerate() {
    if *count == 0 {
      ready.push(stage_index);
    }
  }
  ready
}

fn collect_ready_downstream_stages(
  stage_index: usize,
  adjacency: &[Vec<usize>],
  processing_incoming: &mut [usize],
  ready: &mut Vec<usize>,
) {
  for next_index in &adjacency[stage_index] {
    processing_incoming[*next_index] -= 1;
    if processing_incoming[*next_index] == 0 {
      ready.push(*next_index);
    }
  }
}

fn stream_plan_flow_order(stages: &[StageDefinition], ordered_indices: Vec<usize>) -> Vec<usize> {
  let mut flow_order = Vec::new();
  for stage_index in ordered_indices {
    if matches!(stages[stage_index], StageDefinition::Flow(_)) {
      flow_order.push(stage_index);
    }
  }
  flow_order
}

pub(crate) struct StreamPlanEdge {
  pub(crate) from_port: PortId,
  pub(crate) to_port:   PortId,
  pub(crate) mat:       MatCombine,
}

pub(crate) trait FlowLogic: Send {
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError>;

  fn handles_failures(&self) -> bool {
    false
  }

  fn on_failure(&mut self, error: StreamError) -> Result<FailureAction, StreamError> {
    Ok(FailureAction::Propagate(error))
  }

  fn on_tick(&mut self, tick_count: u64) -> Result<(), StreamError> {
    let _ = tick_count;
    Ok(())
  }

  fn on_async_callback(&mut self) -> Result<Vec<DynValue>, StreamError> {
    Ok(Vec::new())
  }

  fn on_timer(&mut self) -> Result<Vec<DynValue>, StreamError> {
    Ok(Vec::new())
  }

  fn can_accept_input(&self) -> bool {
    true
  }

  fn can_accept_input_while_output_buffered(&self) -> bool {
    false
  }

  fn apply_with_edge(&mut self, edge_index: usize, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let _ = edge_index;
    self.apply(input)
  }

  fn preferred_input_edge_slot(&self) -> Option<usize> {
    None
  }

  fn take_next_output_edge_slot(&mut self) -> Option<usize> {
    None
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

  fn on_downstream_cancel(&mut self) -> Result<DownstreamCancelAction, StreamError> {
    self.on_source_done().map(|()| DownstreamCancelAction::Propagate)
  }

  fn take_shutdown_request(&mut self) -> bool {
    false
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    Ok(Vec::new())
  }

  fn has_pending_output(&self) -> bool {
    false
  }

  fn wants_upstream_drain(&self) -> bool {
    false
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    Ok(())
  }

  fn attach_actor_system(&mut self, system: ActorSystem) {
    drop(system);
  }
}

pub(crate) enum DownstreamCancelAction {
  Drain,
  Propagate,
}

pub(crate) enum FailureAction {
  Propagate(StreamError),
  Resume,
  Complete,
}

fn downcast_value<In>(value: DynValue) -> Result<In, StreamError>
where
  In: Any + Send + 'static, {
  match value.downcast::<In>() {
    | Ok(value) => Ok(*value),
    | Err(_) => Err(StreamError::TypeMismatch),
  }
}
