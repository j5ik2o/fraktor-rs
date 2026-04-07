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
/// Restart log level enum.
mod restart_log_level;
/// Restart log settings.
mod restart_log_settings;
/// Restart and backoff configuration settings.
mod restart_settings;
/// Stream reference serialization support.
mod serialization;
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
/// Unique kill switch for single-stream control.
mod unique_kill_switch;
// stateful_map_concat_accumulator moved to dsl/stateful_map_concat_accumulator
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
use fraktor_utils_core_rs::core::sync::ArcShared;
use r#impl::{
  RestartBackoff, fusing::DemandTracker as InternalDemandTracker,
  validate_positive_argument as internal_validate_positive_argument,
};
pub use io_result::IOResult;
pub use kill_switch::KillSwitch;
pub use kill_switches::KillSwitches;
use materialization::MatCombine;
pub use overflow_strategy::OverflowStrategy;
pub use queue_offer_result::QueueOfferResult;
pub use restart_log_level::RestartLogLevel;
pub use restart_log_settings::RestartLogSettings;
pub use restart_settings::RestartSettings;
use shape::PortId;
pub use shared_kill_switch::SharedKillSwitch;
pub use sink_decision::SinkDecision;
pub use sink_logic::SinkLogic;
pub use source_logic::SourceLogic;
use stage::StageKind;
/// Tracks downstream demand for sink stages.
pub type DemandTracker = InternalDemandTracker;
/// Public alias for stream DSL construction errors.
pub type StreamDslError = r#impl::StreamDslError;
/// Public alias for stream execution errors.
pub type StreamError = r#impl::StreamError;
// StatefulMapConcatAccumulator re-exported via dsl::StatefulMapConcatAccumulator
pub use substream_cancel_strategy::SubstreamCancelStrategy;
pub use supervision_strategy::SupervisionStrategy;
pub use throttle_mode::ThrottleMode;
pub use unique_kill_switch::UniqueKillSwitch;
pub(in crate::core) use unique_kill_switch::{KillSwitchState, KillSwitchStateHandle};

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

impl StreamPlan {
  pub(crate) fn from_parts(
    stages: Vec<StageDefinition>,
    edges: Vec<(PortId, PortId, MatCombine)>,
  ) -> Result<Self, StreamError> {
    if stages.is_empty() || edges.is_empty() {
      return Err(StreamError::InvalidConnection);
    }

    let mut source_indices = Vec::new();
    let mut sink_indices = Vec::new();

    let mut input_ports = Vec::with_capacity(stages.len());
    let mut output_ports = Vec::with_capacity(stages.len());

    for (stage_index, stage) in stages.iter().enumerate() {
      match stage {
        | StageDefinition::Source(_) => {
          source_indices.push(stage_index);
        },
        | StageDefinition::Flow(_) => {},
        | StageDefinition::Sink(_) => {
          sink_indices.push(stage_index);
        },
      }

      if let Some(inlet) = stage.inlet() {
        if input_ports.iter().any(|(port, _)| *port == inlet) {
          return Err(StreamError::InvalidConnection);
        }
        input_ports.push((inlet, stage_index));
      }
      if let Some(outlet) = stage.outlet() {
        if output_ports.iter().any(|(port, _)| *port == outlet) {
          return Err(StreamError::InvalidConnection);
        }
        output_ports.push((outlet, stage_index));
      }
    }

    if source_indices.is_empty() {
      return Err(StreamError::InvalidConnection);
    }
    if sink_indices.is_empty() {
      return Err(StreamError::InvalidConnection);
    }

    let mut incoming = alloc::vec::Vec::with_capacity(stages.len());
    let mut outgoing = alloc::vec::Vec::with_capacity(stages.len());
    let mut adjacency = alloc::vec::Vec::with_capacity(stages.len());

    for _ in 0..stages.len() {
      incoming.push(0_usize);
      outgoing.push(0_usize);
      adjacency.push(alloc::vec::Vec::new());
    }

    let mut plan_edges = alloc::vec::Vec::with_capacity(edges.len());

    for (from, to, mat) in edges {
      let Some(from_stage) = output_ports.iter().find(|(port, _)| *port == from).map(|(_, stage_index)| *stage_index)
      else {
        return Err(StreamError::InvalidConnection);
      };
      let Some(to_stage) = input_ports.iter().find(|(port, _)| *port == to).map(|(_, stage_index)| *stage_index) else {
        return Err(StreamError::InvalidConnection);
      };
      outgoing[from_stage] = outgoing[from_stage].saturating_add(1);
      incoming[to_stage] = incoming[to_stage].saturating_add(1);
      adjacency[from_stage].push(to_stage);
      plan_edges.push(StreamPlanEdge { from_port: from, to_port: to, mat });
    }

    for stage_index in 0..stages.len() {
      match &stages[stage_index] {
        | StageDefinition::Source(_) => {
          if outgoing[stage_index] == 0 {
            return Err(StreamError::InvalidConnection);
          }
        },
        | StageDefinition::Flow(definition) => {
          if incoming[stage_index] == 0 {
            return Err(StreamError::InvalidConnection);
          }
          if let Some(expected_fan_in) = definition.logic.expected_fan_in()
            && incoming[stage_index] != expected_fan_in
          {
            return Err(StreamError::InvalidConnection);
          }
          if outgoing[stage_index] == 0 {
            return Err(StreamError::InvalidConnection);
          }
          if let Some(expected_fan_out) = definition.logic.expected_fan_out()
            && outgoing[stage_index] != expected_fan_out
          {
            return Err(StreamError::InvalidConnection);
          }
        },
        | StageDefinition::Sink(_) => {
          if incoming[stage_index] == 0 {
            return Err(StreamError::InvalidConnection);
          }
        },
      }
    }

    let mut ready = Vec::new();
    for (stage_index, count) in incoming.iter().enumerate() {
      if *count == 0 {
        ready.push(stage_index);
      }
    }

    let mut processing_incoming = incoming;
    let mut ordered_indices = Vec::new();

    while let Some(stage_index) = ready.pop() {
      ordered_indices.push(stage_index);
      for next_index in &adjacency[stage_index] {
        processing_incoming[*next_index] = processing_incoming[*next_index].saturating_sub(1);
        if processing_incoming[*next_index] == 0 {
          ready.push(*next_index);
        }
      }
    }

    if ordered_indices.len() != stages.len() {
      return Err(StreamError::InvalidConnection);
    }

    let mut flow_order = Vec::new();
    for stage_index in ordered_indices {
      if matches!(stages[stage_index], StageDefinition::Flow(_)) {
        flow_order.push(stage_index);
      }
    }

    Ok(Self { stages, edges: plan_edges, source_indices, sink_indices, flow_order, kill_switch_states: Vec::new() })
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
  ) -> Self {
    Self { stages, edges, source_indices, sink_indices, flow_order, kill_switch_states: Vec::new() }
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
