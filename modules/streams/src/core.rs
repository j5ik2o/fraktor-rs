/// Completion polling types.
mod completion;
/// Demand model types.
mod demand;
/// Demand tracking utilities.
mod demand_tracker;
/// Graph-related abstractions.
pub mod graph;
/// Dynamic fan-in/fan-out connectors.
pub mod hub;
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
/// Materialization pipeline.
pub mod mat;
/// Materialization combination kinds.
mod mat_combine;
/// Materialization combination rules.
mod mat_combine_rule;
/// Operator compatibility catalog.
pub mod operator;
/// Restart/backoff configuration.
mod restart_settings;
/// Stream topology shapes and connection points.
pub mod shape;
/// Stage definitions for source, flow, and sink.
pub mod stage;
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
/// Test utilities for stream verification.
pub mod testing;
/// Positive argument validator.
mod validate_positive_argument;

use alloc::{boxed::Box, vec::Vec};
use core::any::{Any, TypeId};

pub use completion::Completion;
pub use demand::Demand;
pub use demand_tracker::DemandTracker;
pub use keep_both::KeepBoth;
pub use keep_left::KeepLeft;
pub use keep_none::KeepNone;
pub use keep_right::KeepRight;
pub use mat_combine::MatCombine;
pub use mat_combine_rule::MatCombineRule;
pub use restart_settings::RestartSettings;
use shape::PortId;
use stage::StageKind;
pub use stream_buffer::StreamBuffer;
pub use stream_buffer_config::StreamBufferConfig;
pub use stream_completion::StreamCompletion;
pub use stream_done::StreamDone;
pub use stream_dsl_error::StreamDslError;
pub use stream_error::StreamError;
pub use stream_not_used::StreamNotUsed;
pub use validate_positive_argument::validate_positive_argument;
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
  supervision: SupervisionStrategy,
  restart:     Option<RestartBackoff>,
  logic:       Box<dyn SourceLogic>,
}

struct FlowDefinition {
  kind:        StageKind,
  inlet:       PortId,
  outlet:      PortId,
  input_type:  TypeId,
  output_type: TypeId,
  mat_combine: MatCombine,
  supervision: SupervisionStrategy,
  restart:     Option<RestartBackoff>,
  logic:       Box<dyn FlowLogic>,
}

struct SinkDefinition {
  kind:        StageKind,
  inlet:       PortId,
  input_type:  TypeId,
  mat_combine: MatCombine,
  supervision: SupervisionStrategy,
  restart:     Option<RestartBackoff>,
  logic:       Box<dyn SinkLogic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SupervisionStrategy {
  Stop,
  Resume,
  Restart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RestartBackoff {
  settings:              RestartSettings,
  restart_count:         usize,
  cooldown_ticks:        u32,
  pending:               bool,
  current_backoff_ticks: u32,
  last_schedule_tick:    u64,
  jitter_state:          u64,
}

impl RestartBackoff {
  const fn new(min_backoff_ticks: u32, max_restarts: usize) -> Self {
    Self::from_settings(RestartSettings::new(min_backoff_ticks, min_backoff_ticks, max_restarts))
  }

  const fn from_settings(settings: RestartSettings) -> Self {
    Self {
      settings,
      restart_count: 0,
      cooldown_ticks: 0,
      pending: false,
      current_backoff_ticks: settings.min_backoff_ticks(),
      last_schedule_tick: 0,
      jitter_state: settings.jitter_seed(),
    }
  }

  const fn is_waiting(&self) -> bool {
    self.pending
  }

  const fn complete_on_max_restarts(self) -> bool {
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
struct StreamPlan {
  stages:            Vec<StageDefinition>,
  edges:             Vec<(PortId, PortId, MatCombine)>,
  kill_switch_state: Option<lifecycle::KillSwitchStateHandle>,
}

impl StreamPlan {
  const fn from_parts(stages: Vec<StageDefinition>, edges: Vec<(PortId, PortId, MatCombine)>) -> Self {
    Self { stages, edges, kill_switch_state: None }
  }

  fn with_shared_kill_switch_state(mut self, kill_switch_state: lifecycle::KillSwitchStateHandle) -> Self {
    self.kill_switch_state = Some(kill_switch_state);
    self
  }

  fn shared_kill_switch_state(&self) -> Option<lifecycle::KillSwitchStateHandle> {
    self.kill_switch_state.clone()
  }
}

trait SourceLogic: Send {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError>;

  fn on_cancel(&mut self) -> Result<(), StreamError> {
    Ok(())
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    Ok(())
  }
}

trait FlowLogic: Send {
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError>;

  fn can_accept_input(&self) -> bool {
    true
  }

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

  fn take_shutdown_request(&mut self) -> bool {
    false
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    Ok(Vec::new())
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    Ok(())
  }
}

enum SinkDecision {
  Continue,
  Complete,
}

trait SinkLogic: Send {
  fn can_accept_input(&self) -> bool {
    true
  }

  fn on_start(&mut self, demand: &mut DemandTracker) -> Result<(), StreamError>;
  fn on_push(&mut self, input: DynValue, demand: &mut DemandTracker) -> Result<SinkDecision, StreamError>;
  fn on_complete(&mut self) -> Result<(), StreamError>;
  fn on_error(&mut self, error: StreamError);

  fn on_restart(&mut self) -> Result<(), StreamError> {
    Ok(())
  }
}

fn downcast_value<In>(value: DynValue) -> Result<In, StreamError>
where
  In: Any + Send + Sync + 'static, {
  match value.downcast::<In>() {
    | Ok(value) => Ok(*value),
    | Err(_) => Err(StreamError::TypeMismatch),
  }
}
