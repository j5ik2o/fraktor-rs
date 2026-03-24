/// Handle for sending elements into an actor-sourced stream.
mod actor_source_ref;
/// Marker attribute for async boundaries in stream graphs.
mod async_boundary_attr;
/// Type-safe attribute trait for stream stage metadata.
mod attribute;
/// Stream attributes for stage and graph metadata.
mod attributes;
/// Bounded queue materialized by source queue stages.
mod bounded_source_queue;
/// Cancellation strategy definitions for stream stages.
mod cancellation_strategy_kind;
/// Completion polling types.
mod completion;
/// Completion strategy for actor-sourced streams.
mod completion_strategy;
/// Compression facade providing gzip and deflate utilities.
#[cfg(feature = "compression")]
mod compression;
/// Supervision decider function type.
mod decider;
/// Per-element delay strategy trait.
mod delay_strategy;
/// Demand model types.
mod demand;
/// Demand tracking utilities.
mod demand_tracker;
/// Dispatcher attribute for stream graph island execution.
mod dispatcher_attribute;
/// Fixed delay strategy implementation.
mod fixed_delay;
/// Byte stream framing utilities.
mod framing;
/// Graph-related abstractions.
pub mod graph;
/// Dynamic fan-in/fan-out connectors.
pub mod hub;
/// Input buffer attribute for stream stage configuration.
mod input_buffer;
/// IO operation result type.
mod io_result;
/// JSON object framing utilities.
mod json_framing;
/// Keep-both materialization rule.
mod keep_both;
/// Keep-left materialization rule.
mod keep_left;
/// Keep-none materialization rule.
mod keep_none;
/// Keep-right materialization rule.
mod keep_right;
/// Stream lifecycle and execution management.
pub mod lifecycle;
/// Linear increasing delay strategy implementation.
mod linear_increasing_delay;
/// Log level definitions for stream attribute configuration.
mod log_level;
/// Log levels attribute for stream stage diagnostics configuration.
mod log_levels;
/// Materialization pipeline.
pub mod mat;
/// Materialization combination kinds.
mod mat_combine;
/// Materialization combination rules.
mod mat_combine_rule;
/// Operator compatibility catalog.
pub mod operator;
/// Overflow strategy definitions compatible with Pekko terminology.
mod overflow_strategy;
/// Queue offer result definitions.
mod queue_offer_result;
/// Log level for restart event diagnostics.
mod restart_log_level;
/// Restart log settings for restart event diagnostics.
mod restart_log_settings;
/// Restart/backoff configuration.
mod restart_settings;
/// Retry flow with exponential backoff for individual element failures.
mod retry_flow;
/// Stream topology shapes and connection points.
pub mod shape;
/// Shared pull handle for queue-based sink materialization.
mod sink_queue;
/// Source queue materialization handle.
mod source_queue;
/// Source queue materialization handle with completion notifications.
mod source_queue_with_complete;
/// Stage definitions for source, flow, and sink.
pub mod stage;
/// Stateful map-concat accumulator trait.
mod stateful_map_concat_accumulator;
/// Stream buffer implementation.
mod stream_buffer;
/// Stream buffer configuration.
mod stream_buffer_config;
/// Stream completion handle.
mod stream_completion;
/// Stream completion marker.
mod stream_done;
/// Stream DSL error definitions.
mod stream_dsl_error;
/// Stream error definitions.
mod stream_error;
/// Stream not-used marker.
mod stream_not_used;
/// Action taken when a subscription times out.
mod subscription_timeout_mode;
/// Subscription timeout settings for stream materializers.
mod subscription_timeout_settings;
/// `split_when` / `split_after` substream cancellation strategy.
mod substream_cancel_strategy;
/// Supervision strategy definitions.
mod supervision_strategy;
/// Test utilities for stream verification.
pub mod testing;
/// Throttle behavior mode.
mod throttle_mode;
/// Positive argument validator.
mod validate_positive_argument;

use alloc::{boxed::Box, vec::Vec};
use core::any::{Any, TypeId};

pub use actor_source_ref::ActorSourceRef;
pub use async_boundary_attr::AsyncBoundaryAttr;
pub use attribute::Attribute;
pub use attributes::Attributes;
pub use bounded_source_queue::BoundedSourceQueue;
pub use cancellation_strategy_kind::CancellationStrategyKind;
pub use completion::Completion;
pub use completion_strategy::CompletionStrategy;
#[cfg(feature = "compression")]
pub use compression::Compression;
pub use decider::Decider;
pub use delay_strategy::DelayStrategy;
pub use demand::Demand;
pub use demand_tracker::DemandTracker;
pub use dispatcher_attribute::DispatcherAttribute;
pub use fixed_delay::FixedDelay;
use fraktor_utils_rs::core::sync::ArcShared;
pub use framing::Framing;
pub use input_buffer::InputBuffer;
pub use io_result::IOResult;
pub use json_framing::JsonFraming;
pub use keep_both::KeepBoth;
pub use keep_left::KeepLeft;
pub use keep_none::KeepNone;
pub use keep_right::KeepRight;
pub use linear_increasing_delay::LinearIncreasingDelay;
pub use log_level::LogLevel;
pub use log_levels::LogLevels;
pub use mat_combine::MatCombine;
pub use mat_combine_rule::MatCombineRule;
pub use overflow_strategy::OverflowStrategy;
pub use queue_offer_result::QueueOfferResult;
pub use restart_log_level::RestartLogLevel;
pub use restart_log_settings::RestartLogSettings;
pub use restart_settings::RestartSettings;
pub use retry_flow::RetryFlow;
use shape::PortId;
pub use sink_queue::SinkQueue;
pub use source_queue::SourceQueue;
pub use source_queue_with_complete::SourceQueueWithComplete;
use stage::StageKind;
pub use stateful_map_concat_accumulator::StatefulMapConcatAccumulator;
pub use stream_buffer::StreamBuffer;
pub use stream_buffer_config::StreamBufferConfig;
pub use stream_completion::StreamCompletion;
pub use stream_done::StreamDone;
pub use stream_dsl_error::StreamDslError;
pub use stream_error::StreamError;
pub use stream_not_used::StreamNotUsed;
pub use subscription_timeout_mode::SubscriptionTimeoutMode;
pub use subscription_timeout_settings::SubscriptionTimeoutSettings;
pub use substream_cancel_strategy::SubstreamCancelStrategy;
pub use supervision_strategy::SupervisionStrategy;
pub use throttle_mode::ThrottleMode;
pub use validate_positive_argument::validate_positive_argument;
pub(crate) type DynValue = Box<dyn Any + Send + 'static>;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RestartBackoff {
  settings:              RestartSettings,
  restart_count:         usize,
  cooldown_ticks:        u32,
  pending:               bool,
  current_backoff_ticks: u32,
  last_schedule_tick:    u64,
  jitter_state:          u64,
}

impl RestartBackoff {
  fn new(min_backoff_ticks: u32, max_restarts: usize) -> Self {
    Self::from_settings(RestartSettings::new(min_backoff_ticks, min_backoff_ticks, max_restarts))
  }

  const fn from_settings(settings: RestartSettings) -> Self {
    let min_backoff_ticks = settings.min_backoff_ticks();
    let jitter_seed = settings.jitter_seed();
    Self {
      settings,
      restart_count: 0,
      cooldown_ticks: 0,
      pending: false,
      current_backoff_ticks: min_backoff_ticks,
      last_schedule_tick: 0,
      jitter_state: jitter_seed,
    }
  }

  const fn is_waiting(&self) -> bool {
    self.pending
  }

  const fn complete_on_max_restarts(&self) -> bool {
    self.settings.complete_on_max_restarts()
  }

  fn schedule(&mut self, now_tick: u64) -> bool {
    self.reset_backoff_if_window_elapsed(now_tick);
    if self.restart_count >= self.settings.max_restarts() {
      return false;
    }
    self.restart_count = self.restart_count.saturating_add(1);
    self.last_schedule_tick = now_tick;
    self.cooldown_ticks = self.next_cooldown_ticks();
    self.pending = true;
    true
  }

  fn tick(&mut self, now_tick: u64) -> bool {
    self.reset_backoff_if_window_elapsed(now_tick);
    if !self.pending {
      return false;
    }
    if self.cooldown_ticks > 0 {
      self.cooldown_ticks = self.cooldown_ticks.saturating_sub(1);
      return false;
    }
    self.pending = false;
    true
  }

  fn next_cooldown_ticks(&mut self) -> u32 {
    let min_ticks = self.settings.min_backoff_ticks();
    let max_ticks = self.settings.max_backoff_ticks();
    let base = self.current_backoff_ticks.max(min_ticks).min(max_ticks);
    let jitter_ticks = self.compute_jitter_ticks(base);
    self.current_backoff_ticks = base.saturating_mul(2).min(max_ticks).max(min_ticks);
    base.saturating_add(jitter_ticks).min(max_ticks)
  }

  fn reset_backoff_if_window_elapsed(&mut self, now_tick: u64) {
    let window = u64::from(self.settings.max_restarts_within_ticks());
    if window == 0 {
      return;
    }
    if now_tick.saturating_sub(self.last_schedule_tick) > window {
      self.current_backoff_ticks = self.settings.min_backoff_ticks();
    }
  }

  fn compute_jitter_ticks(&mut self, base_ticks: u32) -> u32 {
    let factor = u32::from(self.settings.random_factor_permille());
    if factor == 0 || base_ticks == 0 {
      return 0;
    }
    self.jitter_state = self.jitter_state.wrapping_mul(6364136223846793005).wrapping_add(1);
    let ratio_permille = (self.jitter_state >> 32) as u32 % 1001;
    base_ticks.saturating_mul(factor).saturating_mul(ratio_permille) / 1_000_000
  }
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
  kill_switch_states:        Vec<lifecycle::KillSwitchStateHandle>,
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

  fn with_shared_kill_switch_state(mut self, kill_switch_state: lifecycle::KillSwitchStateHandle) -> Self {
    if self.kill_switch_states.iter().any(|existing| ArcShared::ptr_eq(existing, &kill_switch_state)) {
      return self;
    }
    self.kill_switch_states.push(kill_switch_state);
    self
  }

  fn shared_kill_switch_states(&self) -> &[lifecycle::KillSwitchStateHandle] {
    &self.kill_switch_states
  }
}

pub(crate) struct StreamPlanEdge {
  pub(crate) from_port: PortId,
  pub(crate) to_port:   PortId,
  pub(crate) mat:       MatCombine,
}

pub(crate) trait SourceLogic: Send {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError>;

  fn on_cancel(&mut self) -> Result<(), StreamError> {
    Ok(())
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    Ok(())
  }
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

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SinkDecision {
  Continue,
  Complete,
}

pub(crate) enum FailureAction {
  Propagate(StreamError),
  Resume,
  Complete,
}

pub(crate) trait SinkLogic: Send {
  fn can_accept_input(&self) -> bool {
    true
  }

  fn on_start(&mut self, demand: &mut DemandTracker) -> Result<(), StreamError>;
  fn on_push(&mut self, input: DynValue, demand: &mut DemandTracker) -> Result<SinkDecision, StreamError>;
  fn on_complete(&mut self) -> Result<(), StreamError>;
  fn on_error(&mut self, error: StreamError);
  fn on_tick(&mut self, _demand: &mut DemandTracker) -> Result<bool, StreamError> {
    Ok(false)
  }

  fn on_upstream_finish(&mut self) -> Result<bool, StreamError> {
    Ok(false)
  }

  fn has_pending_work(&self) -> bool {
    false
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    Ok(())
  }
}

fn downcast_value<In>(value: DynValue) -> Result<In, StreamError>
where
  In: Any + Send + 'static, {
  match value.downcast::<In>() {
    | Ok(value) => Ok(*value),
    | Err(_) => Err(StreamError::TypeMismatch),
  }
}
