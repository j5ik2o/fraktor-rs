use super::FlowMonitor;
use crate::core::{
  StreamError,
  stage::{FlowMonitorImpl, FlowMonitorState},
};

// --- FlowMonitorImpl: initial state ---

#[test]
fn new_monitor_starts_in_initialized_state() {
  // Given: a newly created flow monitor implementation
  let monitor = FlowMonitorImpl::<u32>::new();

  // When: querying the current state
  let state = monitor.state();

  // Then: the state is Initialized
  assert!(matches!(state, FlowMonitorState::Initialized));
}

// --- state transitions ---

#[test]
fn monitor_transitions_to_received_on_element() {
  // Given: a flow monitor
  let mut monitor = FlowMonitorImpl::<u32>::new();

  // When: updating the state with a received element
  monitor.set_state(FlowMonitorState::Received(42));

  // Then: the state reflects the received value
  assert!(matches!(monitor.state(), FlowMonitorState::Received(42)));
}

#[test]
fn monitor_transitions_to_failed_on_error() {
  // Given: a flow monitor
  let mut monitor = FlowMonitorImpl::<u32>::new();

  // When: setting a failure state
  monitor.set_state(FlowMonitorState::Failed(StreamError::Failed));

  // Then: the state reflects the failure
  assert!(matches!(monitor.state(), FlowMonitorState::Failed(StreamError::Failed)));
}

#[test]
fn monitor_transitions_to_finished_on_completion() {
  // Given: a flow monitor
  let mut monitor = FlowMonitorImpl::<u32>::new();

  // When: setting the finished state
  monitor.set_state(FlowMonitorState::Finished);

  // Then: the state reflects completion
  assert!(matches!(monitor.state(), FlowMonitorState::Finished));
}

#[test]
fn monitor_overwrites_previous_received_value() {
  // Given: a monitor that has received a value
  let mut monitor = FlowMonitorImpl::<u32>::new();
  monitor.set_state(FlowMonitorState::Received(1));

  // When: receiving a new value
  monitor.set_state(FlowMonitorState::Received(2));

  // Then: only the latest value is reflected
  assert!(matches!(monitor.state(), FlowMonitorState::Received(2)));
}

// --- FlowMonitor trait usage ---

#[test]
fn flow_monitor_trait_state_returns_correct_state() {
  // Given: a monitor behind the FlowMonitor trait
  let monitor = FlowMonitorImpl::<u32>::new();
  let trait_ref: &dyn FlowMonitor<u32> = &monitor;

  // When: calling state() via the trait
  let state = trait_ref.state();

  // Then: it returns the correct initial state
  assert!(matches!(state, FlowMonitorState::Initialized));
}
