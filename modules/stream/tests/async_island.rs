//! Integration tests for Pekko-compatible `async()` island splitting.
//!
//! These tests verify the end-to-end behavior of async boundaries that split
//! a stream graph into independently executed islands, following Pekko semantics.
//!
//! NOTE: These tests will not compile until the production implementation is in place.
//! They define the expected behavioral contract for Gate 0.

use fraktor_stream_rs::core::{
  Attributes, StreamNotUsed,
  stage::{Source, flow::Flow},
};

// --- Basic element pass-through across island boundary ---

#[test]
fn async_island_passes_single_element() {
  // Given: a source with a single element and an async boundary
  // The async boundary should split the graph into two islands,
  // but the observable behavior is identical to no boundary.
  let values = Source::single(42_u32).via(Flow::new().r#async()).collect_values().expect("collect_values");

  // Then: the element passes through
  assert_eq!(values, vec![42_u32]);
}

#[test]
fn async_island_passes_large_sequence() {
  // Given: a larger sequence to verify island-boundary channel capacity
  let input: Vec<u32> = (0..100).collect();
  let values = Source::from_iterator(input.clone().into_iter())
    .via(Flow::new().r#async())
    .collect_values()
    .expect("collect_values");

  // Then: all elements arrive in order
  assert_eq!(values, input);
}

// --- Multiple islands ---

#[test]
fn two_async_boundaries_create_three_islands() {
  // Given: a graph with two async boundaries → 3 islands
  let input: Vec<u32> = (1..=10).collect();
  let values = Source::from_iterator(input.clone().into_iter())
    .via(Flow::new().map(|x: u32| x * 2).r#async())
    .via(Flow::new().map(|x: u32| x + 1).r#async())
    .collect_values()
    .expect("collect_values");

  // Then: both transforms are applied, elements arrive in order
  let expected: Vec<u32> = (1..=10).map(|x| x * 2 + 1).collect();
  assert_eq!(values, expected);
}

// --- Async boundary with attributes ---

#[test]
fn async_with_attributes_passes_elements() {
  // Given: a flow with async boundary created via add_attributes
  // This mirrors Pekko's approach where async() just adds attributes
  let values = Source::from_iterator(vec![1_u32, 2, 3].into_iter())
    .via(Flow::new().add_attributes(Attributes::async_boundary()))
    .collect_values()
    .expect("collect_values");

  // Then: elements pass through correctly
  assert_eq!(values, vec![1_u32, 2, 3]);
}

#[test]
fn async_with_dispatcher_attribute_passes_elements() {
  // Given: a flow with async boundary + dispatcher attribute
  let attrs = Attributes::async_boundary().and(Attributes::dispatcher("custom-dispatcher"));
  let values = Source::from_iterator(vec![1_u32, 2, 3].into_iter())
    .via(Flow::new().add_attributes(attrs))
    .collect_values()
    .expect("collect_values");

  // Then: elements pass through correctly (dispatcher is used by materializer)
  assert_eq!(values, vec![1_u32, 2, 3]);
}

#[test]
fn async_with_input_buffer_attribute_passes_elements() {
  // Given: async boundary with custom input buffer size
  let attrs = Attributes::async_boundary().and(Attributes::dispatcher("default")).and(Attributes::input_buffer(32, 32));
  let values = Source::from_iterator(vec![1_u32, 2, 3].into_iter())
    .via(Flow::new().add_attributes(attrs))
    .collect_values()
    .expect("collect_values");

  // Then: elements pass through with the configured buffer size
  assert_eq!(values, vec![1_u32, 2, 3]);
}

// --- Completion propagation ---

#[test]
fn async_island_propagates_normal_completion() {
  // Given: a finite source through an async boundary
  let values = Source::from_iterator(vec![1_u32, 2].into_iter())
    .via(Flow::new().r#async())
    .collect_values()
    .expect("collect_values");

  // Then: the stream completes normally after all elements are emitted
  assert_eq!(values, vec![1_u32, 2]);
}

#[test]
fn async_island_empty_source_completes() {
  // Given: an empty source through an async boundary
  let values =
    Source::<u32, StreamNotUsed>::empty().via(Flow::new().r#async()).collect_values().expect("collect_values");

  // Then: the stream completes with no elements
  assert!(values.is_empty());
}

// --- Composition with other operators ---

#[test]
fn async_island_with_filter_and_map() {
  // Given: filter → async → map pipeline across island boundary
  let values = Source::from_iterator((1_u32..=10).into_iter())
    .via(Flow::new().filter(|x: &u32| x % 2 == 0))
    .via(Flow::new().r#async())
    .via(Flow::new().map(|x: u32| x * 10))
    .collect_values()
    .expect("collect_values");

  // Then: filter runs in island 1, map runs in island 2
  assert_eq!(values, vec![20_u32, 40, 60, 80, 100]);
}

#[test]
fn async_island_with_take() {
  // Given: source → async → take pipeline
  let values = Source::from_iterator((1_u32..=100).into_iter())
    .via(Flow::new().r#async())
    .via(Flow::new().take(3))
    .collect_values()
    .expect("collect_values");

  // Then: only the first 3 elements are taken
  assert_eq!(values, vec![1_u32, 2, 3]);
}

#[test]
fn async_island_with_grouped() {
  // Given: source → async → grouped pipeline
  let values = Source::from_iterator((1_u32..=6).into_iter())
    .via(Flow::new().r#async())
    .via(Flow::new().grouped(2).expect("grouped"))
    .collect_values()
    .expect("collect_values");

  // Then: elements are grouped in pairs after crossing island boundary
  assert_eq!(values, vec![vec![1_u32, 2], vec![3, 4], vec![5, 6]]);
}
