use super::GraphStageFlowContext;
use crate::core::{stage::StageContext, stream_error::StreamError};

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------

#[test]
fn new_context_has_no_input() {
  // Given/When: a freshly constructed context
  let ctx = GraphStageFlowContext::<u32, u32>::new();

  // Then: is_available reports false — no input has been set
  assert!(!ctx.is_available());
}

#[test]
fn new_context_has_no_outputs() {
  // Given/When: a freshly constructed context
  let mut ctx = GraphStageFlowContext::<u32, u32>::new();

  // Then: take_outputs returns an empty vec
  assert!(ctx.take_outputs().is_empty());
}

#[test]
fn new_context_ports_are_open() {
  // Given/When: a freshly constructed context
  let ctx = GraphStageFlowContext::<u32, u32>::new();

  // Then: both ports start open
  assert!(!ctx.is_closed_in());
  assert!(!ctx.is_closed_out());
}

#[test]
fn new_context_has_not_been_pulled() {
  // Given/When: a freshly constructed context
  let ctx = GraphStageFlowContext::<u32, u32>::new();

  // Then: pull has not been called
  assert!(!ctx.has_been_pulled());
}

// ---------------------------------------------------------------------------
// set_input / grab / is_available
// ---------------------------------------------------------------------------

#[test]
fn set_input_makes_element_available() {
  // Given: a context with no input
  let mut ctx = GraphStageFlowContext::<u32, u32>::new();

  // When: setting an input element
  ctx.set_input(42_u32);

  // Then: is_available returns true
  assert!(ctx.is_available());
}

#[test]
fn grab_returns_set_input_and_clears_availability() {
  // Given: a context with input set
  let mut ctx = GraphStageFlowContext::<u32, u32>::new();
  ctx.set_input(99_u32);

  // When: grabbing the input
  let value = ctx.grab();

  // Then: the value matches and is_available becomes false
  assert_eq!(value, 99_u32);
  assert!(!ctx.is_available());
}

#[test]
#[should_panic(expected = "grab called without available input")]
fn grab_panics_when_no_input_available() {
  // Given: a context with no input set
  let mut ctx = GraphStageFlowContext::<u32, u32>::new();

  // When/Then: grabbing panics
  let _value = ctx.grab();
}

// ---------------------------------------------------------------------------
// push / take_outputs
// ---------------------------------------------------------------------------

#[test]
fn push_accumulates_outputs_as_dyn_values() {
  // Given: a context
  let mut ctx = GraphStageFlowContext::<u32, u32>::new();

  // When: pushing two output values
  ctx.push(10_u32);
  ctx.push(20_u32);

  // Then: take_outputs returns them in order as DynValues that downcast to u32
  let outputs = ctx.take_outputs();
  assert_eq!(outputs.len(), 2);
  assert_eq!(*outputs[0].downcast_ref::<u32>().unwrap(), 10_u32);
  assert_eq!(*outputs[1].downcast_ref::<u32>().unwrap(), 20_u32);
}

#[test]
fn take_outputs_clears_buffer() {
  // Given: a context with buffered outputs
  let mut ctx = GraphStageFlowContext::<u32, u32>::new();
  ctx.push(1_u32);
  let _ = ctx.take_outputs();

  // When: taking outputs again
  let outputs = ctx.take_outputs();

  // Then: the second call returns empty
  assert!(outputs.is_empty());
}

// ---------------------------------------------------------------------------
// pull / has_been_pulled
// ---------------------------------------------------------------------------

#[test]
fn pull_sets_has_been_pulled_flag() {
  // Given: a context where pull has not been called
  let mut ctx = GraphStageFlowContext::<u32, u32>::new();
  assert!(!ctx.has_been_pulled());

  // When: pull is called
  ctx.pull();

  // Then: has_been_pulled returns true
  assert!(ctx.has_been_pulled());
}

// ---------------------------------------------------------------------------
// complete / fail
// ---------------------------------------------------------------------------

#[test]
fn complete_marks_context_as_completed() {
  // Given: a running context
  let mut ctx = GraphStageFlowContext::<u32, u32>::new();

  // When: complete is called
  ctx.complete();

  // Then: is_completed returns true
  assert!(ctx.completed);
}

#[test]
fn fail_stores_error_retrievable_via_take_failure() {
  // Given: a running context
  let mut ctx = GraphStageFlowContext::<u32, u32>::new();

  // When: fail is called with an error
  ctx.fail(StreamError::InvalidConnection);

  // Then: take_failure returns the error
  let failure = ctx.take_failure();
  assert_eq!(failure, Some(StreamError::InvalidConnection));
}

#[test]
fn take_failure_clears_stored_error() {
  // Given: a context with a stored failure
  let mut ctx = GraphStageFlowContext::<u32, u32>::new();
  ctx.fail(StreamError::InvalidConnection);
  let _ = ctx.take_failure();

  // When: taking failure again
  let failure = ctx.take_failure();

  // Then: the second call returns None
  assert!(failure.is_none());
}

// ---------------------------------------------------------------------------
// Port close marking
// ---------------------------------------------------------------------------

#[test]
fn mark_input_closed_sets_is_closed_in() {
  // Given: a context with open ports
  let mut ctx = GraphStageFlowContext::<u32, u32>::new();

  // When: marking input closed
  ctx.mark_input_closed();

  // Then: is_closed_in returns true
  assert!(ctx.is_closed_in());
  assert!(!ctx.is_closed_out());
}

#[test]
fn mark_output_closed_sets_is_closed_out() {
  // Given: a context with open ports
  let mut ctx = GraphStageFlowContext::<u32, u32>::new();

  // When: marking output closed
  ctx.mark_output_closed();

  // Then: is_closed_out returns true
  assert!(ctx.is_closed_out());
  assert!(!ctx.is_closed_in());
}

// ---------------------------------------------------------------------------
// async_callback / timer_graph_stage_logic accessors
// ---------------------------------------------------------------------------

#[test]
fn async_callback_is_accessible() {
  // Given: a context
  let ctx = GraphStageFlowContext::<u32, u32>::new();

  // Then: async_callback returns a valid reference
  assert!(ctx.async_callback().is_empty());
}

#[test]
fn timer_schedule_and_advance_works_through_context() {
  // Given: a context
  let mut ctx = GraphStageFlowContext::<u32, u32>::new();

  // When: scheduling a timer through the context
  ctx.schedule_once(1, 2);

  // Then: advancing twice fires the timer key
  let fired_1 = ctx.advance_timers();
  assert!(fired_1.is_empty());
  let fired_2 = ctx.advance_timers();
  assert_eq!(fired_2, alloc::vec![1_u64]);
}
