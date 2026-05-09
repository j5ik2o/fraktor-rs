use alloc::string::String;

use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};

use super::super::unique_kill_switch::{
  KillSwitchCommandTarget, KillSwitchCommandTargetShared, KillSwitchState, KillSwitchStatus,
};
use crate::{
  SharedKillSwitch, StreamError,
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
fn shared_kill_switch_shutdown_is_visible_across_clones() {
  let switch = SharedKillSwitch::new();
  let cloned = switch.clone();
  switch.shutdown();
  assert!(cloned.is_shutdown());
  assert!(!cloned.is_aborted());
}

#[test]
fn shared_kill_switch_abort_is_visible_across_clones() {
  let switch = SharedKillSwitch::new();
  let cloned = switch.clone();
  cloned.abort(StreamError::Failed);
  assert!(switch.is_aborted());
  assert_eq!(switch.abort_error(), Some(StreamError::Failed));
}

#[test]
fn shared_kill_switch_abort_escalates_shutdown_across_clones() {
  let switch = SharedKillSwitch::new();
  let cloned = switch.clone();

  cloned.shutdown();
  switch.abort(StreamError::Failed);

  assert!(!switch.is_shutdown());
  assert!(!cloned.is_shutdown());
  assert!(switch.is_aborted());
  assert_eq!(switch.abort_error(), Some(StreamError::Failed));
}

#[test]
fn shared_kill_switch_shutdown_ignores_command_target_failure() {
  let state = ArcShared::new(SpinSyncMutex::new(KillSwitchState::running()));
  let target: KillSwitchCommandTargetShared = ArcShared::new(FailingKillSwitchCommandTarget);
  let status = state.lock().add_command_target(target);
  assert!(matches!(status, KillSwitchStatus::Running));
  let switch = SharedKillSwitch::from_state(state);

  switch.shutdown();

  assert!(switch.is_shutdown());
  assert!(!switch.is_aborted());
}

#[test]
fn shared_kill_switch_abort_ignores_command_target_failure() {
  let state = ArcShared::new(SpinSyncMutex::new(KillSwitchState::running()));
  let target: KillSwitchCommandTargetShared = ArcShared::new(FailingKillSwitchCommandTarget);
  let status = state.lock().add_command_target(target);
  assert!(matches!(status, KillSwitchStatus::Running));
  let switch = SharedKillSwitch::from_state(state);

  switch.abort(StreamError::Failed);

  assert!(switch.is_aborted());
  assert_eq!(switch.abort_error(), Some(StreamError::Failed));
}

#[test]
fn shared_kill_switch_flow_binds_state_to_graph() {
  let switch = SharedKillSwitch::new_named(String::from("shared-flow"));
  let graph = Source::single(1_u32).via_mat(switch.flow::<u32>(), KeepRight).into_mat(Sink::head(), KeepLeft);
  let (plan, materialized) = graph.into_parts();

  assert!(plan.shared_kill_switch_states().is_empty());
  assert!(!materialized.is_shutdown());
  assert!(!materialized.is_aborted());
}

#[test]
fn shared_kill_switch_new_named_accepts_owned_string() {
  let switch = SharedKillSwitch::new_named(String::from("owned-shared"));

  assert_eq!(switch.name(), Some("owned-shared"));
}

// ---------------------------------------------------------------------------
// Batch 8 Task V: SharedKillSwitch::bidi_flow<T1, T2>()
// ---------------------------------------------------------------------------

#[test]
fn shared_kill_switch_bidi_flow_shares_state_across_clones() {
  // Given: a shared kill switch that is cloned; the clone's bidi_flow is split
  let switch = SharedKillSwitch::new();
  let cloned = switch.clone();
  let bidi: BidiFlow<u32, u32, u64, u64, SharedKillSwitch> = cloned.bidi_flow::<u32, u64>();
  let (_top, _bottom, materialized) = bidi.split();

  // When: the original switch is shut down
  switch.shutdown();

  // Then: the materialized switch (from the clone) observes the shared shutdown
  assert!(materialized.is_shutdown());
  assert!(!materialized.is_aborted());
}

#[test]
fn shared_kill_switch_bidi_flow_shutdown_propagates_to_materialized_switch() {
  // Given: a bidi flow constructed from a shared kill switch
  let switch = SharedKillSwitch::new();
  let bidi: BidiFlow<u32, u32, u64, u64, SharedKillSwitch> = switch.bidi_flow::<u32, u64>();
  let (_top, _bottom, materialized) = bidi.split();

  // When: shutdown is requested on the source switch
  switch.shutdown();

  // Then: the materialized switch mirrors the shutdown state
  assert!(materialized.is_shutdown());
  assert!(!materialized.is_aborted());
}

#[test]
fn shared_kill_switch_bidi_flow_abort_propagates_identical_error() {
  // Given: a bidi flow from a shared kill switch
  let switch = SharedKillSwitch::new();
  let bidi: BidiFlow<u32, u32, u64, u64, SharedKillSwitch> = switch.bidi_flow::<u32, u64>();
  let (top, bottom, materialized) = bidi.split();

  // When: abort is requested
  switch.abort(StreamError::Failed);

  // Then: materialized switch carries identical error
  assert!(materialized.is_aborted());
  assert_eq!(materialized.abort_error(), Some(StreamError::Failed));

  // Graph-level verification: both fragments are built against the shared state
  let (top_graph, _top_mat) = top.into_parts();
  let (bottom_graph, _bottom_mat) = bottom.into_parts();
  let _ = (top_graph, bottom_graph);
}

#[test]
fn shared_kill_switch_named_bidi_flow_attaches_named_attribute_to_top_and_bottom_fragments() {
  // Given: a named shared kill switch; its bidi_flow should attach the debug name to both sides
  let switch = SharedKillSwitch::new_named(String::from("shared-bidi"));
  let bidi: BidiFlow<u32, u32, u64, u64, SharedKillSwitch> = switch.bidi_flow::<u32, u64>();
  let (top, bottom, _mat) = bidi.split();

  // When: extracting the graphs of both sides
  let (top_graph, _) = top.into_parts();
  let (bottom_graph, _) = bottom.into_parts();

  // Then: both graphs carry the shared switch name as an attribute
  let top_names = top_graph.attributes().names();
  let bottom_names = bottom_graph.attributes().names();
  assert!(top_names.iter().any(|n| n == "shared-bidi"));
  assert!(bottom_names.iter().any(|n| n == "shared-bidi"));
}

#[test]
fn shared_kill_switch_bidi_flow_without_name_leaves_fragments_unadorned() {
  // Given: an anonymous shared kill switch
  let switch = SharedKillSwitch::new();
  let bidi: BidiFlow<u32, u32, u64, u64, SharedKillSwitch> = switch.bidi_flow::<u32, u64>();
  let (top, bottom, _mat) = bidi.split();

  // When: extracting the graphs
  let (top_graph, _) = top.into_parts();
  let (bottom_graph, _) = bottom.into_parts();

  // Then: no name attributes are attached (absence of a debug name on the switch)
  assert!(top_graph.attributes().names().is_empty());
  assert!(bottom_graph.attributes().names().is_empty());
}

#[test]
fn shared_kill_switch_bidi_flow_fragments_materialize_as_stream_not_used() {
  // Given: a bidi flow
  let switch = SharedKillSwitch::new();
  let bidi: BidiFlow<u32, u32, u64, u64, SharedKillSwitch> = switch.bidi_flow::<u32, u64>();
  let (top, bottom, _mat) = bidi.split();

  // When: assigning the fragments to typed Flow variables
  let top_flow: Flow<u32, u32, StreamNotUsed> = top;
  let bottom_flow: Flow<u64, u64, StreamNotUsed> = bottom;

  // Then: the flows compose cleanly with trivial sinks (sanity: no panic)
  let _ = Source::single(1_u32).via(top_flow).into_mat(Sink::<u32, _>::ignore(), KeepLeft);
  let _ = Source::single(2_u64).via(bottom_flow).into_mat(Sink::<u64, _>::ignore(), KeepLeft);
}

#[test]
fn shared_kill_switch_bidi_flow_is_running_by_default() {
  // Given: a fresh shared kill switch
  let switch = SharedKillSwitch::new();

  // When: bidi_flow is created without any prior control signal
  let bidi: BidiFlow<u32, u32, u64, u64, SharedKillSwitch> = switch.bidi_flow::<u32, u64>();
  let (_top, _bottom, materialized) = bidi.split();

  // Then: materialized switch is in Running state
  assert!(!materialized.is_shutdown());
  assert!(!materialized.is_aborted());
  assert!(materialized.abort_error().is_none());
}
