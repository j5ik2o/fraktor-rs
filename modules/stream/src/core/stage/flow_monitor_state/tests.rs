use crate::core::{stage::FlowMonitorState, stream_error::StreamError};

// --- variant construction ---

#[test]
fn initialized_is_default_state() {
  // Given/When: creating an initialized state
  let state: FlowMonitorState<u32> = FlowMonitorState::Initialized;

  // Then: it matches the Initialized variant
  assert!(matches!(state, FlowMonitorState::Initialized));
}

#[test]
fn received_holds_message() {
  // Given: a received message value
  let state = FlowMonitorState::Received(42_u32);

  // Then: the value is accessible
  assert!(matches!(state, FlowMonitorState::Received(42)));
}

#[test]
fn failed_holds_error() {
  // Given: a stream error
  let error = StreamError::Failed;
  let state: FlowMonitorState<u32> = FlowMonitorState::Failed(error.clone());

  // Then: the error is accessible
  assert!(matches!(state, FlowMonitorState::Failed(e) if e == error));
}

#[test]
fn finished_represents_completion() {
  // Given/When: creating a finished state
  let state: FlowMonitorState<u32> = FlowMonitorState::Finished;

  // Then: it matches the Finished variant
  assert!(matches!(state, FlowMonitorState::Finished));
}

// --- equality ---

#[test]
fn same_variants_are_equal() {
  assert_eq!(FlowMonitorState::<u32>::Initialized, FlowMonitorState::Initialized);
  assert_eq!(FlowMonitorState::Received(10_u32), FlowMonitorState::Received(10_u32));
  assert_eq!(FlowMonitorState::<u32>::Finished, FlowMonitorState::Finished);
}

#[test]
fn different_variants_are_not_equal() {
  assert_ne!(FlowMonitorState::<u32>::Initialized, FlowMonitorState::Finished);
  assert_ne!(FlowMonitorState::Received(1_u32), FlowMonitorState::Received(2_u32));
  assert_ne!(FlowMonitorState::<u32>::Initialized, FlowMonitorState::Received(0));
}

// --- debug formatting ---

#[test]
fn debug_format_is_readable() {
  // Given: each variant
  let initialized = FlowMonitorState::<u32>::Initialized;
  let received = FlowMonitorState::Received(99_u32);
  let finished = FlowMonitorState::<u32>::Finished;

  // Then: debug formatting does not panic and produces non-empty output
  let debug_init = alloc::format!("{:?}", initialized);
  let debug_recv = alloc::format!("{:?}", received);
  let debug_fin = alloc::format!("{:?}", finished);

  assert!(!debug_init.is_empty());
  assert!(!debug_recv.is_empty());
  assert!(!debug_fin.is_empty());
}

// --- clone ---

#[test]
fn clone_preserves_variant() {
  let original = FlowMonitorState::Received(42_u32);
  let cloned = original.clone();
  assert_eq!(original, cloned);
}
