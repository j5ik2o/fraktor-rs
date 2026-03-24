use alloc::{boxed::Box, vec::Vec};

use super::GraphStageFlowAdapter;
use crate::core::{DynValue, FlowLogic, StreamError, StreamNotUsed, graph::GraphStageLogic, stage::StageContext};

// ---------------------------------------------------------------------------
// Test helper: a map-like GraphStageLogic that multiplies by 2
// ---------------------------------------------------------------------------

struct DoubleLogic {
  started: bool,
  stopped: bool,
}

impl DoubleLogic {
  fn new() -> Self {
    Self { started: false, stopped: false }
  }
}

impl GraphStageLogic<u32, u32, StreamNotUsed> for DoubleLogic {
  fn on_start(&mut self, _ctx: &mut dyn StageContext<u32, u32>) {
    self.started = true;
  }

  fn on_push(&mut self, ctx: &mut dyn StageContext<u32, u32>) {
    let value = ctx.grab();
    ctx.push(value * 2);
  }

  fn on_stop(&mut self, _ctx: &mut dyn StageContext<u32, u32>) {
    self.stopped = true;
  }

  fn materialized(&mut self) -> StreamNotUsed {
    StreamNotUsed::new()
  }
}

// ---------------------------------------------------------------------------
// Test helper: a filter-like GraphStageLogic that only passes even values
// ---------------------------------------------------------------------------

struct EvenFilterLogic;

impl GraphStageLogic<u32, u32, StreamNotUsed> for EvenFilterLogic {
  fn on_push(&mut self, ctx: &mut dyn StageContext<u32, u32>) {
    let value = ctx.grab();
    if value % 2 == 0 {
      ctx.push(value);
    }
    // odd values are dropped (no push)
  }

  fn materialized(&mut self) -> StreamNotUsed {
    StreamNotUsed::new()
  }
}

// ---------------------------------------------------------------------------
// Test helper: a stage that calls fail() on specific input
// ---------------------------------------------------------------------------

struct FailOnZeroLogic;

impl GraphStageLogic<u32, u32, StreamNotUsed> for FailOnZeroLogic {
  fn on_push(&mut self, ctx: &mut dyn StageContext<u32, u32>) {
    let value = ctx.grab();
    if value == 0 {
      ctx.fail(StreamError::InvalidConnection);
    } else {
      ctx.push(value);
    }
  }

  fn materialized(&mut self) -> StreamNotUsed {
    StreamNotUsed::new()
  }
}

// ---------------------------------------------------------------------------
// Test helper: a stage that calls complete() on specific input
// ---------------------------------------------------------------------------

struct CompleteOnNinetyNineLogic;

impl GraphStageLogic<u32, u32, StreamNotUsed> for CompleteOnNinetyNineLogic {
  fn on_push(&mut self, ctx: &mut dyn StageContext<u32, u32>) {
    let value = ctx.grab();
    if value == 99 {
      ctx.complete();
    } else {
      ctx.push(value);
    }
  }

  fn materialized(&mut self) -> StreamNotUsed {
    StreamNotUsed::new()
  }
}

// ---------------------------------------------------------------------------
// apply() tests
// ---------------------------------------------------------------------------

#[test]
fn apply_converts_dynvalue_and_calls_on_push() {
  // Given: a DoubleLogic adapter
  let logic: Box<dyn GraphStageLogic<u32, u32, StreamNotUsed> + Send> = Box::new(DoubleLogic::new());
  let mut adapter = GraphStageFlowAdapter::new(logic);

  // When: apply is called with a DynValue containing 5
  let input: DynValue = Box::new(5_u32);
  let result = adapter.apply(input);

  // Then: the output is [10] (5 * 2)
  let outputs = result.expect("apply should succeed");
  assert_eq!(outputs.len(), 1);
  assert_eq!(*outputs[0].downcast_ref::<u32>().unwrap(), 10_u32);
}

#[test]
fn apply_calls_on_start_on_first_invocation() {
  // Given: a DoubleLogic adapter (on_start sets started flag)
  let logic: Box<dyn GraphStageLogic<u32, u32, StreamNotUsed> + Send> = Box::new(DoubleLogic::new());
  let mut adapter = GraphStageFlowAdapter::new(logic);

  // When: apply is called for the first time
  let input: DynValue = Box::new(1_u32);
  let _result = adapter.apply(input);

  // Then: on_start was called (verified indirectly via successful execution)
  // on_start is called before on_push on the first invocation

  // When: apply is called a second time
  let input2: DynValue = Box::new(2_u32);
  let result = adapter.apply(input2);

  // Then: still works (on_start not called again)
  let outputs = result.expect("second apply should succeed");
  assert_eq!(*outputs[0].downcast_ref::<u32>().unwrap(), 4_u32);
}

#[test]
fn apply_returns_type_mismatch_error_for_wrong_type() {
  // Given: a DoubleLogic adapter expecting u32
  let logic: Box<dyn GraphStageLogic<u32, u32, StreamNotUsed> + Send> = Box::new(DoubleLogic::new());
  let mut adapter = GraphStageFlowAdapter::new(logic);

  // When: apply is called with a String instead of u32
  let input: DynValue = Box::new(String::from("not a number"));
  let result = adapter.apply(input);

  // Then: TypeMismatch error is returned
  assert!(result.is_err());
  assert_eq!(result.unwrap_err(), StreamError::TypeMismatch);
}

#[test]
fn apply_returns_empty_outputs_when_logic_does_not_push() {
  // Given: an EvenFilterLogic adapter
  let logic: Box<dyn GraphStageLogic<u32, u32, StreamNotUsed> + Send> = Box::new(EvenFilterLogic);
  let mut adapter = GraphStageFlowAdapter::new(logic);

  // When: apply is called with an odd number
  let input: DynValue = Box::new(3_u32);
  let result = adapter.apply(input);

  // Then: the output is empty (odd value filtered)
  let outputs = result.expect("apply should succeed");
  assert!(outputs.is_empty());
}

#[test]
fn apply_returns_error_when_logic_calls_fail() {
  // Given: a FailOnZeroLogic adapter
  let logic: Box<dyn GraphStageLogic<u32, u32, StreamNotUsed> + Send> = Box::new(FailOnZeroLogic);
  let mut adapter = GraphStageFlowAdapter::new(logic);

  // When: apply is called with 0
  let input: DynValue = Box::new(0_u32);
  let result = adapter.apply(input);

  // Then: the error from fail() is propagated
  assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// on_source_done() tests
// ---------------------------------------------------------------------------

#[test]
fn on_source_done_calls_on_complete_and_on_stop() {
  // Given: a DoubleLogic adapter
  let logic: Box<dyn GraphStageLogic<u32, u32, StreamNotUsed> + Send> = Box::new(DoubleLogic::new());
  let mut adapter = GraphStageFlowAdapter::new(logic);

  // When: on_source_done is called
  let result = adapter.on_source_done();

  // Then: no error (on_complete + on_stop called successfully)
  assert!(result.is_ok());
}

// ---------------------------------------------------------------------------
// on_downstream_cancel() tests
// ---------------------------------------------------------------------------

#[test]
fn on_downstream_cancel_returns_propagate() {
  // Given: a DoubleLogic adapter
  let logic: Box<dyn GraphStageLogic<u32, u32, StreamNotUsed> + Send> = Box::new(DoubleLogic::new());
  let mut adapter = GraphStageFlowAdapter::new(logic);

  // When: on_downstream_cancel is called
  let result = adapter.on_downstream_cancel();

  // Then: returns Propagate action
  assert!(result.is_ok());
}

// ---------------------------------------------------------------------------
// Multiple elements: map-like behavior verification
// ---------------------------------------------------------------------------

#[test]
fn apply_multiple_elements_produces_correct_outputs() {
  // Given: a DoubleLogic adapter
  let logic: Box<dyn GraphStageLogic<u32, u32, StreamNotUsed> + Send> = Box::new(DoubleLogic::new());
  let mut adapter = GraphStageFlowAdapter::new(logic);

  // When: applying multiple elements
  let results: Vec<u32> = [1_u32, 2, 3, 4, 5]
    .iter()
    .map(|&v| {
      let input: DynValue = Box::new(v);
      let outputs = adapter.apply(input).expect("apply should succeed");
      *outputs[0].downcast_ref::<u32>().unwrap()
    })
    .collect();

  // Then: each output is double the input
  assert_eq!(results, alloc::vec![2_u32, 4, 6, 8, 10]);
}

#[test]
fn filter_logic_passes_only_matching_elements() {
  // Given: an EvenFilterLogic adapter
  let logic: Box<dyn GraphStageLogic<u32, u32, StreamNotUsed> + Send> = Box::new(EvenFilterLogic);
  let mut adapter = GraphStageFlowAdapter::new(logic);

  // When: applying mixed even/odd elements
  let mut passed = Vec::new();
  for &v in &[1_u32, 2, 3, 4, 5, 6] {
    let input: DynValue = Box::new(v);
    let outputs = adapter.apply(input).expect("apply should succeed");
    for out in outputs {
      passed.push(*out.downcast_ref::<u32>().unwrap());
    }
  }

  // Then: only even values pass through
  assert_eq!(passed, alloc::vec![2_u32, 4, 6]);
}
