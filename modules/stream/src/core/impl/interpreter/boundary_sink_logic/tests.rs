//! Unit tests for `BoundarySinkLogic` — upstream island's exit stage.
//!
//! NOTE: These tests will not compile until the production implementation is in place.
//! They define the expected behavioral contract for Gate 0, step C-2a.

use super::{
  super::island_boundary::{BoundaryState, IslandBoundaryShared},
  BoundarySinkLogic,
};
use crate::core::{DemandTracker, DynValue, SinkDecision, SinkLogic, StreamError};

// --- on_start requests initial demand ---

#[test]
fn on_start_requests_demand() {
  // Given: a BoundarySinkLogic connected to a shared boundary
  let boundary = IslandBoundaryShared::new(16);
  let mut logic = BoundarySinkLogic::new(boundary);
  let mut demand = DemandTracker::new();

  // When: starting
  logic.on_start(&mut demand).expect("on_start");

  // Then: demand is requested (at least 1)
  assert!(demand.pending() > 0);
}

// --- on_push forwards element to boundary ---

#[test]
fn on_push_forwards_element_to_boundary() {
  // Given: a started sink logic
  let boundary = IslandBoundaryShared::new(16);
  let mut logic = BoundarySinkLogic::new(boundary.clone());
  let mut demand = DemandTracker::new();
  logic.on_start(&mut demand).expect("on_start");

  // When: pushing an element
  let value: DynValue = Box::new(42_u32);
  let decision = logic.on_push(value, &mut demand).expect("on_push");

  // Then: element is in the boundary buffer
  assert_eq!(decision, SinkDecision::Continue);
  let (pulled, _state) = boundary.try_pull_with_state();
  let pulled = pulled.expect("pull");
  let v = *pulled.downcast::<u32>().expect("downcast");
  assert_eq!(v, 42_u32);
}

#[test]
fn on_push_requests_more_demand_after_success() {
  // Given: a started sink logic
  let boundary = IslandBoundaryShared::new(16);
  let mut logic = BoundarySinkLogic::new(boundary);
  let mut demand = DemandTracker::new();
  logic.on_start(&mut demand).expect("on_start");
  let initial_pending = demand.pending();

  // When: pushing an element (consumes 1 demand, requests 1 more)
  let value: DynValue = Box::new(1_u32);
  logic.on_push(value, &mut demand).expect("on_push");

  // Then: demand is still available (refilled after push)
  assert!(demand.pending() >= initial_pending);
}

// --- Backpressure: boundary full → pending ---

#[test]
fn on_push_to_full_boundary_stores_pending() {
  // Given: a boundary with capacity 1, already full
  let boundary = IslandBoundaryShared::new(1);
  let v: DynValue = Box::new(0_u32);
  boundary.try_push_with_state(v).expect("fill");
  let mut logic = BoundarySinkLogic::new(boundary.clone());
  let mut demand = DemandTracker::new();
  logic.on_start(&mut demand).expect("on_start");

  // When: pushing while boundary is full
  let value: DynValue = Box::new(99_u32);
  let decision = logic.on_push(value, &mut demand).expect("on_push");

  // Then: sink continues (element is held internally as pending)
  assert_eq!(decision, SinkDecision::Continue);
  assert!(logic.has_pending_work());
  assert!(!logic.can_accept_input());
}

#[test]
fn can_accept_input_returns_false_while_pending_element_exists() {
  let boundary = IslandBoundaryShared::new(1);
  let v: DynValue = Box::new(0_u32);
  boundary.try_push_with_state(v).expect("fill");
  let mut logic = BoundarySinkLogic::new(boundary);
  let mut demand = DemandTracker::new();
  logic.on_start(&mut demand).expect("on_start");

  let pending_value: DynValue = Box::new(99_u32);
  logic.on_push(pending_value, &mut demand).expect("on_push");

  assert!(!logic.can_accept_input());
}

#[test]
fn on_push_while_pending_returns_would_block_without_overwriting_existing_pending() {
  let boundary = IslandBoundaryShared::new(1);
  let v: DynValue = Box::new(0_u32);
  boundary.try_push_with_state(v).expect("fill");
  let mut logic = BoundarySinkLogic::new(boundary.clone());
  let mut demand = DemandTracker::new();
  logic.on_start(&mut demand).expect("on_start");

  let first_pending: DynValue = Box::new(99_u32);
  logic.on_push(first_pending, &mut demand).expect("first on_push");

  let second_input: DynValue = Box::new(123_u32);
  let result = logic.on_push(second_input, &mut demand);
  assert_eq!(result, Err(StreamError::WouldBlock));

  let _ = boundary.try_pull_with_state();
  let progressed = logic.on_tick(&mut demand).expect("on_tick");
  assert!(progressed);

  let (pulled, _state) = boundary.try_pull_with_state();
  let pulled = pulled.expect("pull");
  let value = *pulled.downcast::<u32>().expect("downcast");
  assert_eq!(value, 99_u32);
}

#[test]
fn on_tick_retries_pending_push() {
  // Given: a full boundary with a pending element
  let boundary = IslandBoundaryShared::new(1);
  let v: DynValue = Box::new(0_u32);
  boundary.try_push_with_state(v).expect("fill");
  let mut logic = BoundarySinkLogic::new(boundary.clone());
  let mut demand = DemandTracker::new();
  logic.on_start(&mut demand).expect("on_start");

  let pending_value: DynValue = Box::new(99_u32);
  logic.on_push(pending_value, &mut demand).expect("on_push");
  assert!(logic.has_pending_work());

  // When: we free the boundary and tick
  let _ = boundary.try_pull_with_state(); // free one slot
  let progress = logic.on_tick(&mut demand).expect("on_tick");

  // Then: pending element is now pushed, progress reported
  assert!(progress);
  assert!(!logic.has_pending_work());

  // And the element is in the boundary
  let (pulled, _state) = boundary.try_pull_with_state();
  let pulled = pulled.expect("pull");
  let v = *pulled.downcast::<u32>().expect("downcast");
  assert_eq!(v, 99_u32);
}

#[test]
fn on_tick_without_pending_returns_no_progress() {
  // Given: a sink logic with no pending work
  let boundary = IslandBoundaryShared::new(16);
  let mut logic = BoundarySinkLogic::new(boundary);
  let mut demand = DemandTracker::new();
  logic.on_start(&mut demand).expect("on_start");

  // When: ticking with nothing pending
  let progress = logic.on_tick(&mut demand).expect("on_tick");

  // Then: no progress
  assert!(!progress);
}

#[test]
fn on_tick_with_still_full_boundary_returns_no_progress() {
  // Given: a full boundary with a pending element
  let boundary = IslandBoundaryShared::new(1);
  let v: DynValue = Box::new(0_u32);
  boundary.try_push_with_state(v).expect("fill");
  let mut logic = BoundarySinkLogic::new(boundary);
  let mut demand = DemandTracker::new();
  logic.on_start(&mut demand).expect("on_start");

  let pending_value: DynValue = Box::new(99_u32);
  logic.on_push(pending_value, &mut demand).expect("on_push");

  // When: ticking without freeing boundary
  let progress = logic.on_tick(&mut demand).expect("on_tick");

  // Then: no progress (boundary still full)
  assert!(!progress);
  assert!(logic.has_pending_work());
}

// --- Completion propagation ---

#[test]
fn on_complete_marks_boundary_as_completed() {
  // Given: a started sink logic
  let boundary = IslandBoundaryShared::new(16);
  let mut logic = BoundarySinkLogic::new(boundary.clone());
  let mut demand = DemandTracker::new();
  logic.on_start(&mut demand).expect("on_start");

  // When: completing
  logic.on_complete().expect("on_complete");

  // Then: boundary state is Completed
  assert_eq!(boundary.state(), BoundaryState::Completed);
}

#[test]
fn on_complete_with_pending_defers_until_flush() {
  let boundary = IslandBoundaryShared::new(1);
  let v: DynValue = Box::new(0_u32);
  boundary.try_push_with_state(v).expect("fill");
  let mut logic = BoundarySinkLogic::new(boundary.clone());
  let mut demand = DemandTracker::new();
  logic.on_start(&mut demand).expect("on_start");

  let pending_value: DynValue = Box::new(99_u32);
  logic.on_push(pending_value, &mut demand).expect("on_push");
  logic.on_complete().expect("on_complete");

  assert_eq!(boundary.state(), BoundaryState::Open);
  assert!(logic.has_pending_work());

  let _ = boundary.try_pull_with_state();
  let progress = logic.on_tick(&mut demand).expect("on_tick");
  assert!(progress);
  assert_eq!(boundary.state(), BoundaryState::Completed);
}

// --- Error propagation ---

#[test]
fn on_error_marks_boundary_as_failed() {
  // Given: a started sink logic
  let boundary = IslandBoundaryShared::new(16);
  let mut logic = BoundarySinkLogic::new(boundary.clone());
  let mut demand = DemandTracker::new();
  logic.on_start(&mut demand).expect("on_start");

  // When: error occurs
  logic.on_error(StreamError::Failed);

  // Then: boundary state is Failed
  match boundary.state() {
    | BoundaryState::Failed(err) => assert_eq!(err, StreamError::Failed),
    | other => panic!("expected Failed, got {other:?}"),
  }
}

#[test]
fn on_error_with_pending_defers_until_flush() {
  let boundary = IslandBoundaryShared::new(1);
  let v: DynValue = Box::new(0_u32);
  boundary.try_push_with_state(v).expect("fill");
  let mut logic = BoundarySinkLogic::new(boundary.clone());
  let mut demand = DemandTracker::new();
  logic.on_start(&mut demand).expect("on_start");

  let pending_value: DynValue = Box::new(99_u32);
  logic.on_push(pending_value, &mut demand).expect("on_push");
  logic.on_error(StreamError::Failed);

  assert_eq!(boundary.state(), BoundaryState::Open);
  assert!(logic.has_pending_work());

  let _ = boundary.try_pull_with_state();
  let progress = logic.on_tick(&mut demand).expect("on_tick");
  assert!(progress);
  match boundary.state() {
    | BoundaryState::Failed(err) => assert_eq!(err, StreamError::Failed),
    | other => panic!("expected Failed, got {other:?}"),
  }
}

#[test]
fn on_tick_returns_stream_detached_when_boundary_is_completed_while_pending() {
  let boundary = IslandBoundaryShared::new(1);
  let v: DynValue = Box::new(0_u32);
  boundary.try_push_with_state(v).expect("fill");
  let mut logic = BoundarySinkLogic::new(boundary.clone());
  let mut demand = DemandTracker::new();
  logic.on_start(&mut demand).expect("on_start");

  let pending_value: DynValue = Box::new(99_u32);
  logic.on_push(pending_value, &mut demand).expect("on_push");
  boundary.complete();

  let result = logic.on_tick(&mut demand);
  assert_eq!(result, Err(StreamError::StreamDetached));
  assert!(!logic.has_pending_work());
}
