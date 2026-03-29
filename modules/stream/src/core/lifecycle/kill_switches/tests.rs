use crate::core::{
  buffer::StreamBufferConfig,
  dsl::{Sink, Source},
  lifecycle::{KillSwitches, Stream, StreamState},
  materialization::{KeepBoth, KeepLeft, KeepRight},
};

#[test]
fn kill_switches_shared_returns_shared_kill_switch() {
  let switch = KillSwitches::shared(alloc::string::String::from("shared-switch"));
  assert_eq!(switch.name(), Some("shared-switch"));
  assert!(!switch.is_shutdown());
  assert!(!switch.is_aborted());
}

#[test]
fn kill_switches_single_returns_unique_kill_switch_flow() {
  let graph = Source::single(1_u32).via_mat(KillSwitches::single::<u32>(), KeepRight).into_mat(Sink::head(), KeepLeft);
  let (_plan, switch) = graph.into_parts();
  assert!(!switch.is_shutdown());
  assert!(!switch.is_aborted());
}

#[test]
fn kill_switches_single_flow_materializes_bound_state() {
  let switch_flow = KillSwitches::single::<u32>();
  let graph = Source::single(1_u32).via_mat(switch_flow, KeepRight).into_mat(Sink::head(), KeepLeft);
  let (plan, switch) = graph.into_parts();
  assert!(plan.shared_kill_switch_states().is_empty());
  assert!(!switch.is_shutdown());
  assert!(!switch.is_aborted());
}

#[test]
fn kill_switches_single_bidi_returns_bidi_flow_with_kill_switch() {
  let bidi = KillSwitches::single_bidi::<u32, u32>();
  let (_, _, switch) = bidi.split();
  assert!(!switch.is_shutdown());
  assert!(!switch.is_aborted());
}

#[test]
fn kill_switches_single_bidi_shutdown_propagates_to_switch() {
  let bidi = KillSwitches::single_bidi::<u32, u32>();
  let (_top, _bottom, switch) = bidi.split();
  switch.shutdown();
  assert!(switch.is_shutdown());
}

#[test]
fn kill_switches_single_allows_multiple_distinct_flow_switches() {
  let graph = Source::repeat(1_u32)
    .via_mat(KillSwitches::single::<u32>(), KeepRight)
    .via_mat(KillSwitches::single::<u32>(), KeepBoth)
    .into_mat(Sink::ignore(), KeepLeft);
  let (plan, (first_switch, second_switch)) = graph.into_parts();

  assert!(plan.shared_kill_switch_states().is_empty());

  let mut stream = Stream::new(plan, StreamBufferConfig::default());
  stream.start().expect("start");

  for _ in 0..3 {
    let _ = stream.drive();
  }
  assert_eq!(stream.state(), StreamState::Running);

  second_switch.shutdown();
  for _ in 0..4 {
    let _ = stream.drive();
    if stream.state().is_terminal() {
      break;
    }
  }

  assert_eq!(stream.state(), StreamState::Completed);
  assert!(!first_switch.is_shutdown());
}
