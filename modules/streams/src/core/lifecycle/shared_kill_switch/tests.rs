use crate::core::{
  KeepLeft, KeepRight, StreamError,
  lifecycle::SharedKillSwitch,
  stage::{Sink, Source},
};

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
fn shared_kill_switch_keeps_first_control_signal_across_clones() {
  let switch = SharedKillSwitch::new();
  let cloned = switch.clone();

  cloned.shutdown();
  switch.abort(StreamError::Failed);

  assert!(switch.is_shutdown());
  assert!(cloned.is_shutdown());
  assert!(!switch.is_aborted());
  assert_eq!(switch.abort_error(), None);
}

#[test]
fn shared_kill_switch_flow_binds_state_to_graph() {
  let switch = SharedKillSwitch::new_named(alloc::string::String::from("shared-flow"));
  let graph = Source::single(1_u32).via_mat(switch.flow::<u32>(), KeepRight).to_mat(Sink::head(), KeepLeft);
  let (plan, materialized) = graph.into_parts();

  assert!(plan.shared_kill_switch_states().is_empty());
  assert!(!materialized.is_shutdown());
  assert!(!materialized.is_aborted());
}

#[test]
fn shared_kill_switch_new_named_accepts_owned_string() {
  let switch = SharedKillSwitch::new_named(alloc::string::String::from("owned-shared"));

  assert_eq!(switch.name(), Some("owned-shared"));
}
