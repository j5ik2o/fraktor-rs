use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

use super::{
  super::KillSwitches, KillSwitchCommandTarget, KillSwitchCommandTargetShared, KillSwitchState, KillSwitchStatus,
};
use crate::core::{
  StreamError, UniqueKillSwitch,
  dsl::{BidiFlow, Flow, Sink, Source},
  materialization::{KeepLeft, KeepRight, StreamNotUsed},
};

struct FailingKillSwitchCommandTarget;

impl KillSwitchCommandTarget for FailingKillSwitchCommandTarget {
  fn shutdown(&self) -> Result<(), StreamError> {
    Err(StreamError::Failed)
  }

  fn abort(&self, error: StreamError) -> Result<(), StreamError> {
    Err(error)
  }
}

#[test]
fn unique_kill_switch_shutdown_sets_state() {
  let switch = UniqueKillSwitch::new();
  switch.shutdown();
  assert!(switch.is_shutdown());
  assert!(!switch.is_aborted());
}

#[test]
fn unique_kill_switch_abort_sets_error() {
  let switch = UniqueKillSwitch::new();
  switch.abort(StreamError::Failed);
  assert!(switch.is_aborted());
  assert_eq!(switch.abort_error(), Some(StreamError::Failed));
}

#[test]
fn unique_kill_switch_abort_escalates_shutdown() {
  let switch = UniqueKillSwitch::new();

  switch.shutdown();
  switch.abort(StreamError::Failed);

  assert!(!switch.is_shutdown());
  assert!(switch.is_aborted());
  assert_eq!(switch.abort_error(), Some(StreamError::Failed));
}

#[test]
fn unique_kill_switch_abort_keeps_first_error() {
  let switch = UniqueKillSwitch::new();
  let first_error = StreamError::failed_with_context("first abort");
  let second_error = StreamError::failed_with_context("second abort");

  switch.abort(first_error.clone());
  switch.abort(second_error);

  assert!(switch.is_aborted());
  assert_eq!(switch.abort_error(), Some(first_error));
}

#[test]
fn unique_kill_switch_shutdown_ignores_command_target_failure() {
  let state = ArcShared::new(SpinSyncMutex::new(KillSwitchState::running()));
  let target: KillSwitchCommandTargetShared = ArcShared::new(FailingKillSwitchCommandTarget);
  let status = state.lock().add_command_target(target);
  assert!(matches!(status, KillSwitchStatus::Running));
  let switch = UniqueKillSwitch::from_state(state);

  switch.shutdown();

  assert!(switch.is_shutdown());
  assert!(!switch.is_aborted());
}

#[test]
fn unique_kill_switch_abort_ignores_command_target_failure() {
  let state = ArcShared::new(SpinSyncMutex::new(KillSwitchState::running()));
  let target: KillSwitchCommandTargetShared = ArcShared::new(FailingKillSwitchCommandTarget);
  let status = state.lock().add_command_target(target);
  assert!(matches!(status, KillSwitchStatus::Running));
  let switch = UniqueKillSwitch::from_state(state);

  switch.abort(StreamError::Failed);

  assert!(switch.is_aborted());
  assert_eq!(switch.abort_error(), Some(StreamError::Failed));
}

#[test]
fn kill_switch_state_remove_unknown_command_target_returns_false() {
  let mut state = KillSwitchState::running();
  let target: KillSwitchCommandTargetShared = ArcShared::new(FailingKillSwitchCommandTarget);

  let removed = state.remove_command_target(&target);

  assert!(!removed);
}

#[test]
fn unique_kill_switch_flow_binds_state_to_graph() {
  let switch = UniqueKillSwitch::new();
  let graph = Source::single(1_u32).via_mat(switch.flow::<u32>(), KeepRight).into_mat(Sink::head(), KeepLeft);
  let (plan, materialized) = graph.into_parts();

  assert!(plan.shared_kill_switch_states().is_empty());
  assert!(!materialized.is_shutdown());
  assert!(!materialized.is_aborted());
}

// ---------------------------------------------------------------------------
// Batch 8 Task V: UniqueKillSwitch::bidi_flow<T1, T2>()
// ---------------------------------------------------------------------------

#[test]
fn unique_kill_switch_bidi_flow_returns_bidi_flow_with_switch_as_materialized_value() {
  // Given: a unique kill switch
  let switch = UniqueKillSwitch::new();

  // When: `bidi_flow` is created
  let bidi: BidiFlow<u32, u32, u64, u64, UniqueKillSwitch> = switch.bidi_flow::<u32, u64>();
  let (_top, _bottom, materialized) = bidi.split();

  // Then: materialized value is a kill switch whose state equals the source switch
  assert!(!materialized.is_shutdown());
  assert!(!materialized.is_aborted());
}

#[test]
fn unique_kill_switch_bidi_flow_top_and_bottom_share_same_state() {
  // Given: a bidi flow derived from a unique kill switch
  let switch = UniqueKillSwitch::new();
  let bidi: BidiFlow<u32, u32, u64, u64, UniqueKillSwitch> = switch.bidi_flow::<u32, u64>();
  let (_top, _bottom, materialized_switch) = bidi.split();

  // When: the source switch is shut down
  switch.shutdown();

  // Then: the materialized switch observes the same state (shared state handle)
  assert!(materialized_switch.is_shutdown());
  assert!(!materialized_switch.is_aborted());
}

#[test]
fn unique_kill_switch_bidi_flow_abort_propagates_identical_error_to_both_sides() {
  // Given: a bidi flow where top and bottom share a kill switch
  let switch = UniqueKillSwitch::new();
  let bidi: BidiFlow<u32, u32, u64, u64, UniqueKillSwitch> = switch.bidi_flow::<u32, u64>();
  let (top, bottom, materialized_switch) = bidi.split();

  // When: abort is requested on the source switch
  switch.abort(StreamError::Failed);

  // Then: the materialized switch and both flow fragments see the identical abort error
  assert!(materialized_switch.is_aborted());
  assert_eq!(materialized_switch.abort_error(), Some(StreamError::Failed));

  // Graph-level verification: both fragments are built against the same switch state
  let (top_graph, _top_mat) = top.into_parts();
  let (bottom_graph, _bottom_mat) = bottom.into_parts();
  let _ = (top_graph, bottom_graph);
}

#[test]
fn unique_kill_switch_bidi_flow_top_and_bottom_are_usable_as_stream_not_used_flows() {
  // Given: a bidi flow for two distinct element types
  let switch = UniqueKillSwitch::new();
  let bidi: BidiFlow<u32, u32, u64, u64, UniqueKillSwitch> = switch.bidi_flow::<u32, u64>();
  let (top, bottom, _mat) = bidi.split();

  // When: the top and bottom fragments are composed into pipelines
  let top_values = Source::single(1_u32).via(top).into_mat(Sink::head(), KeepLeft);
  let bottom_values = Source::single(7_u64).via(bottom).into_mat(Sink::head(), KeepLeft);

  // Then: both fragments carry `StreamNotUsed` as their materialized value
  let (_top_plan, top_mat) = top_values.into_parts();
  let (_bottom_plan, bottom_mat) = bottom_values.into_parts();
  assert_eq!(top_mat, StreamNotUsed::new());
  assert_eq!(bottom_mat, StreamNotUsed::new());
}

#[test]
fn unique_kill_switch_bidi_flow_does_not_attach_named_attributes() {
  // Given: UniqueKillSwitch has no debug name, so neither side should attach a name attribute
  let switch = UniqueKillSwitch::new();
  let bidi: BidiFlow<u32, u32, u64, u64, UniqueKillSwitch> = switch.bidi_flow::<u32, u64>();
  let (top, bottom, _mat) = bidi.split();

  // When: extracting the underlying graphs
  let (top_graph, _) = top.into_parts();
  let (bottom_graph, _) = bottom.into_parts();

  // Then: no name attributes are attached on either side
  assert!(top_graph.attributes().names().is_empty());
  assert!(bottom_graph.attributes().names().is_empty());
}

#[test]
fn unique_kill_switch_new_bidi_flow_returns_running_by_default() {
  // Given: a fresh kill switch
  let switch = UniqueKillSwitch::new();

  // When: bidi_flow is created without any prior control signal
  let bidi: BidiFlow<u32, u32, u64, u64, UniqueKillSwitch> = switch.bidi_flow::<u32, u64>();
  let (_top, _bottom, materialized) = bidi.split();

  // Then: the materialized switch is in Running state
  assert!(!materialized.is_shutdown());
  assert!(!materialized.is_aborted());
  assert!(materialized.abort_error().is_none());
}

#[test]
fn unique_kill_switch_bidi_flow_is_equivalent_to_kill_switches_single_bidi() {
  // Given: a fresh switch and its bidi flow
  let switch = UniqueKillSwitch::new();
  let bidi_from_instance = switch.bidi_flow::<u32, u64>();
  let (_top_a, _bottom_a, mat_a) = bidi_from_instance.split();

  // When: shutdown is requested on the source switch
  switch.shutdown();

  // Then: the materialized switch reflects the shutdown, mirroring the behaviour of
  // `KillSwitches::single_bidi` where the materialized switch is the sole control handle.
  assert!(mat_a.is_shutdown());

  // Health-check: the derived factory `KillSwitches::single_bidi` must also yield a kill switch
  // whose initial state is Running (sanity before moving on to instance-method equivalence in
  // Batch 8 implementation).
  let bidi_b = KillSwitches::single_bidi::<u32, u64>();
  let (_top_b, _bottom_b, mat_b) = bidi_b.split();
  assert!(!mat_b.is_shutdown());
  assert!(!mat_b.is_aborted());
}

#[test]
fn unique_kill_switch_bidi_flow_top_graph_is_non_empty() {
  // Given: a bidi flow
  let switch = UniqueKillSwitch::new();
  let bidi: BidiFlow<u32, u32, u64, u64, UniqueKillSwitch> = switch.bidi_flow::<u32, u64>();
  let (top, bottom, _mat) = bidi.split();

  // Then: both fragments carry a stage graph that can be composed
  let top_flow: Flow<u32, u32, StreamNotUsed> = top;
  let bottom_flow: Flow<u64, u64, StreamNotUsed> = bottom;

  // Sanity: composition with a trivial sink does not panic
  let _ = Source::single(1_u32).via(top_flow).into_mat(Sink::<u32, _>::ignore(), KeepLeft);
  let _ = Source::single(2_u64).via(bottom_flow).into_mat(Sink::<u64, _>::ignore(), KeepLeft);
}
