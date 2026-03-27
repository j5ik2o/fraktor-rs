use crate::core::{
  StreamError,
  lifecycle::UniqueKillSwitch,
  mat::{KeepLeft, KeepRight},
  stage::{Sink, Source},
};

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
fn unique_kill_switch_keeps_first_control_signal() {
  let switch = UniqueKillSwitch::new();

  switch.shutdown();
  switch.abort(StreamError::Failed);

  assert!(switch.is_shutdown());
  assert!(!switch.is_aborted());
  assert_eq!(switch.abort_error(), None);
}

#[test]
fn unique_kill_switch_flow_binds_state_to_graph() {
  let switch = UniqueKillSwitch::new();
  let graph = Source::single(1_u32).via_mat(switch.flow::<u32>(), KeepRight).to_mat(Sink::head(), KeepLeft);
  let (plan, materialized) = graph.into_parts();

  assert!(plan.shared_kill_switch_states().is_empty());
  assert!(!materialized.is_shutdown());
  assert!(!materialized.is_aborted());
}
