//! Unit tests for `BoundarySourceLogic` — downstream island's entry stage.
//!
//! NOTE: These tests will not compile until the production implementation is in place.
//! They define the expected behavioral contract for Gate 0, step C-2b.

use super::{
  super::island_boundary::{BoundaryState, IslandBoundaryShared},
  BoundarySourceLogic,
};
use crate::{DynValue, SourceLogic, StreamError};

// --- Basic pull ---

#[test]
fn pull_returns_element_from_boundary() {
  // Given: a boundary with one element
  let boundary = IslandBoundaryShared::new(16);
  let v: DynValue = Box::new(42_u32);
  boundary.try_push_with_state(v).expect("push");
  let mut logic = BoundarySourceLogic::new(boundary);

  // When: pulling
  let result = logic.pull().expect("pull");

  // Then: the element is returned
  assert!(result.is_some());
  let value = *result.unwrap().downcast::<u32>().expect("downcast");
  assert_eq!(value, 42_u32);
}

#[test]
fn pull_from_empty_open_boundary_returns_would_block() {
  // Given: an empty, open boundary
  let boundary = IslandBoundaryShared::new(16);
  let mut logic = BoundarySourceLogic::new(boundary);

  // When: pulling
  let result = logic.pull();

  // Then: WouldBlock (interpreter skips this source for the current tick)
  assert!(result.is_err());
  assert_eq!(result.unwrap_err(), StreamError::WouldBlock);
}

#[test]
fn on_cancel_marks_boundary_as_downstream_cancelled() {
  // Given: an open boundary observed by the downstream island
  let boundary = IslandBoundaryShared::new(16);
  let mut logic = BoundarySourceLogic::new(boundary.clone());

  // When: downstream cancellation reaches the boundary source
  logic.on_cancel().expect("cancel");

  // Then: downstream cancellation is distinguishable from upstream completion
  let (value, state) = boundary.try_pull_with_state();
  assert!(value.is_none());
  assert_eq!(state, BoundaryState::DownstreamCancelled);
}

#[test]
fn pull_from_downstream_cancelled_boundary_returns_none() {
  let boundary = IslandBoundaryShared::new(16);
  boundary.cancel_downstream();
  let mut logic = BoundarySourceLogic::new(boundary);

  let result = logic.pull().expect("cancelled boundary should complete source");

  assert!(result.is_none());
}

// --- FIFO ordering ---

#[test]
fn pull_delivers_elements_in_fifo_order() {
  // Given: a boundary with multiple elements
  let boundary = IslandBoundaryShared::new(16);
  for i in 0_u32..5 {
    let v: DynValue = Box::new(i);
    boundary.try_push_with_state(v).expect("push");
  }
  let mut logic = BoundarySourceLogic::new(boundary);

  // Then: elements are pulled in FIFO order
  for expected in 0_u32..5 {
    let result = logic.pull().expect("pull");
    let value = *result.expect("some").downcast::<u32>().expect("downcast");
    assert_eq!(value, expected);
  }
}

// --- Completion propagation ---

#[test]
fn pull_from_empty_completed_boundary_returns_none() {
  // Given: a completed boundary with no remaining elements
  let boundary = IslandBoundaryShared::new(16);
  boundary.complete();
  let mut logic = BoundarySourceLogic::new(boundary);

  // When: pulling
  let result = logic.pull().expect("pull");

  // Then: None (signals stream completion to the downstream interpreter)
  assert!(result.is_none());
}

#[test]
fn pull_drains_remaining_elements_before_completion() {
  // Given: a boundary with one element, then completed
  let boundary = IslandBoundaryShared::new(16);
  let v: DynValue = Box::new(10_u32);
  boundary.try_push_with_state(v).expect("push");
  boundary.complete();
  let mut logic = BoundarySourceLogic::new(boundary);

  // When: first pull gets the buffered element
  let first = logic.pull().expect("pull");
  assert!(first.is_some());
  let value = *first.unwrap().downcast::<u32>().expect("downcast");
  assert_eq!(value, 10_u32);

  // Then: second pull returns None (completed)
  let second = logic.pull().expect("pull");
  assert!(second.is_none());
}

// --- Error propagation ---

#[test]
fn pull_from_empty_failed_boundary_returns_error() {
  // Given: a failed boundary with no remaining elements
  let boundary = IslandBoundaryShared::new(16);
  boundary.fail(StreamError::Failed);
  let mut logic = BoundarySourceLogic::new(boundary);

  // When: pulling
  let result = logic.pull();

  // Then: error is propagated
  assert!(result.is_err());
  assert_eq!(result.unwrap_err(), StreamError::Failed);
}

#[test]
fn pull_drains_elements_before_error() {
  // Given: a boundary with one element, then failed
  let boundary = IslandBoundaryShared::new(16);
  let v: DynValue = Box::new(7_u32);
  boundary.try_push_with_state(v).expect("push");
  boundary.fail(StreamError::Failed);
  let mut logic = BoundarySourceLogic::new(boundary);

  // When: first pull gets the buffered element
  let first = logic.pull().expect("pull");
  assert!(first.is_some());
  let value = *first.unwrap().downcast::<u32>().expect("downcast");
  assert_eq!(value, 7_u32);

  // Then: second pull returns the error
  let second = logic.pull();
  assert!(second.is_err());
  assert_eq!(second.unwrap_err(), StreamError::Failed);
}

// --- Interleaved push/pull (shared boundary) ---

#[test]
fn interleaved_push_and_pull_via_shared_boundary() {
  // Given: a shared boundary used by both source and sink sides
  let boundary = IslandBoundaryShared::new(2);
  let mut logic = BoundarySourceLogic::new(boundary.clone());

  // Initially empty → returns WouldBlock (boundary is Open, no data yet)
  assert_eq!(logic.pull().unwrap_err(), StreamError::WouldBlock);

  // Push from the "sink side"
  let v: DynValue = Box::new(1_u32);
  boundary.try_push_with_state(v).expect("push");

  // Pull from the "source side"
  let result = logic.pull().expect("pull");
  let value = *result.expect("some").downcast::<u32>().expect("downcast");
  assert_eq!(value, 1_u32);

  // Push two more, then complete
  let v1: DynValue = Box::new(2_u32);
  let v2: DynValue = Box::new(3_u32);
  boundary.try_push_with_state(v1).expect("push");
  boundary.try_push_with_state(v2).expect("push");
  boundary.complete();

  // Pull both
  let r1 = logic.pull().expect("pull").expect("some");
  let r2 = logic.pull().expect("pull").expect("some");
  assert_eq!(*r1.downcast::<u32>().expect("downcast"), 2_u32);
  assert_eq!(*r2.downcast::<u32>().expect("downcast"), 3_u32);

  // Final pull → completion
  assert!(logic.pull().expect("pull").is_none());
}
