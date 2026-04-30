//! Unit tests for `IslandBoundary` — bounded buffer between islands.
//!
//! NOTE: These tests will not compile until the production implementation is in place.
//! They define the expected behavioral contract for Gate 0, step C-1.

extern crate std;

use core::any::Any;
use std::{thread, vec::Vec};

use super::{BoundaryState, IslandBoundary, IslandBoundaryShared};
use crate::core::r#impl::StreamError;

impl IslandBoundary {
  fn len(&self) -> usize {
    self.buffer.len()
  }

  fn is_empty(&self) -> bool {
    self.buffer.is_empty()
  }

  fn state(&self) -> &BoundaryState {
    &self.state
  }
}

impl IslandBoundaryShared {
  pub(crate) fn state(&self) -> BoundaryState {
    self.inner.lock().state.clone()
  }
}

// --- Construction ---

#[test]
fn new_boundary_has_zero_length() {
  // Given: a fresh boundary with capacity 16
  let boundary = IslandBoundary::new(16);

  // Then: it contains no elements
  assert_eq!(boundary.len(), 0);
  assert!(boundary.is_empty());
}

#[test]
fn new_boundary_is_open() {
  // Given: a fresh boundary
  let boundary = IslandBoundary::new(16);

  // Then: state is Open
  assert_eq!(*boundary.state(), BoundaryState::Open);
}

// --- try_push / try_pull basic ---

#[test]
fn push_then_pull_returns_same_value() {
  // Given: an empty boundary
  let mut boundary = IslandBoundary::new(16);

  // When: pushing a value
  let value: Box<dyn Any + Send + 'static> = Box::new(42_u32);
  let result = boundary.try_push(value);

  // Then: push succeeds
  assert!(result.is_ok());
  assert_eq!(boundary.len(), 1);

  // When: pulling the value
  let pulled = boundary.try_pull();

  // Then: we get the value back
  assert!(pulled.is_some());
  let pulled_value = pulled.unwrap().downcast::<u32>().expect("downcast");
  assert_eq!(*pulled_value, 42_u32);
  assert!(boundary.is_empty());
}

#[test]
fn pull_from_empty_boundary_returns_none() {
  // Given: an empty boundary
  let mut boundary = IslandBoundary::new(16);

  // When: pulling
  let result = boundary.try_pull();

  // Then: None (no element available)
  assert!(result.is_none());
}

// --- FIFO ordering ---

#[test]
fn elements_are_delivered_in_fifo_order() {
  // Given: a boundary with several pushed elements
  let mut boundary = IslandBoundary::new(16);

  for i in 0_u32..5 {
    let value: Box<dyn Any + Send + 'static> = Box::new(i);
    boundary.try_push(value).expect("push");
  }

  // Then: elements come out in order
  for expected in 0_u32..5 {
    let pulled = boundary.try_pull().expect("pull");
    let value = *pulled.downcast::<u32>().expect("downcast");
    assert_eq!(value, expected);
  }
}

// --- Capacity / backpressure ---

#[test]
fn push_to_full_boundary_returns_err_with_value() {
  // Given: a boundary with capacity 2
  let mut boundary = IslandBoundary::new(2);

  // Fill it up
  let v1: Box<dyn Any + Send + 'static> = Box::new(1_u32);
  let v2: Box<dyn Any + Send + 'static> = Box::new(2_u32);
  boundary.try_push(v1).expect("push 1");
  boundary.try_push(v2).expect("push 2");
  assert_eq!(boundary.len(), 2);

  // When: pushing a third value
  let v3: Box<dyn Any + Send + 'static> = Box::new(3_u32);
  let result = boundary.try_push(v3);

  // Then: push fails, returning the value back
  assert!(result.is_err());
  let returned = result.unwrap_err();
  let returned_value = *returned.downcast::<u32>().expect("downcast");
  assert_eq!(returned_value, 3_u32);
  assert_eq!(boundary.len(), 2);
}

#[test]
fn push_succeeds_after_pull_frees_capacity() {
  // Given: a full boundary (capacity 1)
  let mut boundary = IslandBoundary::new(1);
  let v1: Box<dyn Any + Send + 'static> = Box::new(1_u32);
  boundary.try_push(v1).expect("push");

  // When: pulling frees a slot, then pushing again
  let _ = boundary.try_pull().expect("pull");
  let v2: Box<dyn Any + Send + 'static> = Box::new(2_u32);
  let result = boundary.try_push(v2);

  // Then: push succeeds
  assert!(result.is_ok());
}

// --- Completion propagation ---

#[test]
fn complete_transitions_state_to_completed() {
  // Given: an open boundary
  let mut boundary = IslandBoundary::new(16);

  // When: completing
  boundary.complete();

  // Then: state is Completed
  assert_eq!(*boundary.state(), BoundaryState::Completed);
}

#[test]
fn pull_from_empty_completed_boundary_indicates_completion() {
  // Given: a completed boundary with no remaining elements
  let mut boundary = IslandBoundary::new(16);
  boundary.complete();

  // When: pulling
  let result = boundary.try_pull();

  // Then: None (no elements), and state is Completed (caller should check)
  assert!(result.is_none());
  assert_eq!(*boundary.state(), BoundaryState::Completed);
}

#[test]
fn remaining_elements_are_drained_after_complete() {
  // Given: a boundary with elements, then completed
  let mut boundary = IslandBoundary::new(16);
  let v1: Box<dyn Any + Send + 'static> = Box::new(10_u32);
  boundary.try_push(v1).expect("push");
  boundary.complete();

  // When: pulling
  let pulled = boundary.try_pull();

  // Then: buffered elements are still available
  assert!(pulled.is_some());
  let value = *pulled.unwrap().downcast::<u32>().expect("downcast");
  assert_eq!(value, 10_u32);

  // And next pull returns None with Completed state
  assert!(boundary.try_pull().is_none());
  assert_eq!(*boundary.state(), BoundaryState::Completed);
}

// --- Error propagation ---

#[test]
fn fail_transitions_state_to_failed() {
  // Given: an open boundary
  let mut boundary = IslandBoundary::new(16);

  // When: failing with an error
  boundary.fail(StreamError::Failed);

  // Then: state is Failed with the error
  match boundary.state() {
    | BoundaryState::Failed(err) => assert_eq!(*err, StreamError::Failed),
    | other => panic!("expected Failed, got {other:?}"),
  }
}

#[test]
fn remaining_elements_are_drained_before_error() {
  // Given: a boundary with elements, then failed
  let mut boundary = IslandBoundary::new(16);
  let v1: Box<dyn Any + Send + 'static> = Box::new(42_u32);
  boundary.try_push(v1).expect("push");
  boundary.fail(StreamError::Failed);

  // When: pulling
  let pulled = boundary.try_pull();

  // Then: buffered elements are still available before error surfaces
  assert!(pulled.is_some());
  let value = *pulled.unwrap().downcast::<u32>().expect("downcast");
  assert_eq!(value, 42_u32);

  // And next pull returns None with Failed state
  assert!(boundary.try_pull().is_none());
  match boundary.state() {
    | BoundaryState::Failed(err) => assert_eq!(*err, StreamError::Failed),
    | other => panic!("expected Failed, got {other:?}"),
  }
}

// --- State transition is one-way ---

#[test]
fn complete_after_complete_is_idempotent() {
  // Given: a completed boundary
  let mut boundary = IslandBoundary::new(16);
  boundary.complete();

  // When: completing again
  boundary.complete();

  // Then: still Completed
  assert_eq!(*boundary.state(), BoundaryState::Completed);
}

#[test]
fn push_after_complete_is_rejected() {
  // Given: a completed boundary
  let mut boundary = IslandBoundary::new(16);
  boundary.complete();

  // 実行: push を試みる
  let v: Box<dyn Any + Send + 'static> = Box::new(1_u32);
  let result = boundary.try_push(v);

  // 検証: push が拒否され、元の値がそのまま返却される
  match result {
    | Err(returned) => match returned.downcast::<u32>() {
      | Ok(returned_value) => assert_eq!(*returned_value, 1_u32),
      | Err(_) => panic!("返却値は元の型を保持すべき"),
    },
    | Ok(()) => panic!("完了後の push は拒否されるべき"),
  }
}

// --- IslandBoundaryShared ---

#[test]
fn shared_boundary_is_clone() {
  // Given: a shared boundary
  let shared = IslandBoundaryShared::new(16);

  // When: cloning
  let shared2 = shared.clone();

  // Then: both references point to the same boundary (push on one, pull on other)
  let v: Box<dyn Any + Send + 'static> = Box::new(99_u32);
  shared.try_push_with_state(v).expect("push");
  let (pulled, state) = shared2.try_pull_with_state();
  let value = *pulled.expect("pull").downcast::<u32>().expect("downcast");
  assert_eq!(value, 99_u32);
  assert_eq!(state, BoundaryState::Open);
}

#[test]
fn shared_boundary_preserves_values_under_concurrent_push_pull() {
  // Given: a shared boundary accessed by an upstream and downstream island
  let shared = IslandBoundaryShared::new(8);
  let producer_boundary = shared.clone();
  let consumer_boundary = shared.clone();

  // When: one side pushes while the other side pulls
  let received = thread::scope(|scope| {
    let producer = scope.spawn(move || {
      for next in 0_u32..128 {
        let mut value: Box<dyn Any + Send + 'static> = Box::new(next);
        loop {
          match producer_boundary.try_push_with_state(value) {
            | Ok(()) => break,
            | Err((rejected, BoundaryState::Open)) => {
              value = rejected;
              thread::yield_now();
            },
            | Err((_rejected, state)) => panic!("boundary should remain open while producing: {state:?}"),
          }
        }
      }
      producer_boundary.complete();
    });

    let consumer = scope.spawn(move || {
      let mut received = Vec::new();
      loop {
        let (value, state) = consumer_boundary.try_pull_with_state();
        match value {
          | Some(value) => received.push(*value.downcast::<u32>().expect("u32")),
          | None if state == BoundaryState::Completed => break received,
          | None if state == BoundaryState::Open => thread::yield_now(),
          | None => panic!("unexpected boundary state while consuming: {state:?}"),
        }
      }
    });

    producer.join().expect("producer thread");
    consumer.join().expect("consumer thread")
  });

  // Then: every pushed value is observed once and the terminal state is consistent
  assert_eq!(received.len(), 128);
  for expected in 0_u32..128 {
    assert_eq!(received[expected as usize], expected);
  }
  assert_eq!(shared.state(), BoundaryState::Completed);
}
