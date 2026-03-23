use alloc::{vec, vec::Vec};

use crate::core::{
  StreamNotUsed,
  stage::{FlowWithContext, Source, flow::Flow},
};

#[test]
fn should_map_output_preserving_context() {
  let fwc: FlowWithContext<i32, &str, usize, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, &str)| v)).map(|s: &str| s.len());
  let values = Source::from(vec![(1_i32, "hello"), (2, "world")]).via(fwc.as_flow()).collect_values().unwrap();
  assert_eq!(values, vec![(1, 5), (2, 5)]);
}

#[test]
fn should_filter_by_value_preserving_context() {
  let fwc: FlowWithContext<i32, i32, i32, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, i32)| v)).filter(|v: &i32| *v > 0);
  let values = Source::from(vec![(1_i32, 10), (2, -5), (3, 20)]).via(fwc.as_flow()).collect_values().unwrap();
  assert_eq!(values, vec![(1, 10), (3, 20)]);
}

#[test]
fn should_map_context() {
  // Ctx=i32, Ctx2=i64 — different types ensure map_context cannot be a no-op
  let fwc: FlowWithContext<i32, &str, &str, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, &str)| v));
  // forward and reverse are NOT inverses: output ctx differs from input ctx
  let mapped = fwc.map_context(|ctx: i32| i64::from(ctx) * 10, |ctx2: i64| (ctx2 as i32) - 1);
  // Input: (10_i64, "a"), (20_i64, "b")
  // → reverse(10) = 9, reverse(20) = 19
  // → inner (identity): (9, "a"), (19, "b")
  // → forward(9) = 90, forward(19) = 190
  let values = Source::from(vec![(10_i64, "a"), (20_i64, "b")]).via(mapped.as_flow()).collect_values().unwrap();
  assert_eq!(values, vec![(90_i64, "a"), (190_i64, "b")]);
}

#[test]
fn should_compose_via() {
  let fwc1: FlowWithContext<i32, &str, &str, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, &str)| v));
  let fwc2: FlowWithContext<i32, &str, usize, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|(ctx, s): (i32, &str)| (ctx, s.len())));
  let composed = fwc1.via(fwc2);
  let values = Source::from(vec![(1_i32, "hello"), (2, "hi")]).via(composed.as_flow()).collect_values().unwrap();
  assert_eq!(values, vec![(1, 5), (2, 2)]);
}

// --- map_concat tests ---

#[test]
fn map_concat_expands_elements_preserving_context() {
  // Given: a FlowWithContext that expands each string into its chars
  let fwc: FlowWithContext<i32, &str, &str, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, &str)| v));
  let expanded = fwc.map_concat(|s: &str| s.chars().map(|c| c as u32).collect::<Vec<_>>());

  // When: elements are pushed through
  let values = Source::from(vec![(1_i32, "ab"), (2, "c")]).via(expanded.as_flow()).collect_values().unwrap();

  // Then: each expanded element gets the same context as the original
  assert_eq!(values, vec![(1, 97), (1, 98), (2, 99)]);
}

#[test]
fn map_concat_empty_expansion_drops_element() {
  // Given: a FlowWithContext that returns empty iterator for some elements
  let fwc: FlowWithContext<i32, i32, i32, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, i32)| v));
  let expanded = fwc.map_concat(|v: i32| if v > 0 { vec![v, v * 10] } else { vec![] });

  // When: some elements produce empty iterators
  let values = Source::from(vec![(1_i32, 5), (2, -1), (3, 3)]).via(expanded.as_flow()).collect_values().unwrap();

  // Then: elements with empty expansions are dropped, others expand with same context
  assert_eq!(values, vec![(1, 5), (1, 50), (3, 3), (3, 30)]);
}

// --- filter_not tests ---

#[test]
fn filter_not_passes_elements_where_predicate_is_false() {
  // Given: a FlowWithContext that rejects positive values
  let fwc: FlowWithContext<i32, i32, i32, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, i32)| v));
  let filtered = fwc.filter_not(|v: &i32| *v > 0);

  // When: elements are pushed through
  let values =
    Source::from(vec![(1_i32, 10), (2, -5), (3, 0), (4, 20)]).via(filtered.as_flow()).collect_values().unwrap();

  // Then: only elements where predicate is false pass through with context preserved
  assert_eq!(values, vec![(2, -5), (3, 0)]);
}

#[test]
fn filter_not_passes_all_when_predicate_always_false() {
  // Given: a predicate that is always false
  let fwc: FlowWithContext<i32, i32, i32, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, i32)| v));
  let filtered = fwc.filter_not(|_: &i32| false);

  // When: elements are pushed through
  let values = Source::from(vec![(1_i32, 10), (2, 20)]).via(filtered.as_flow()).collect_values().unwrap();

  // Then: all elements pass through
  assert_eq!(values, vec![(1, 10), (2, 20)]);
}

// --- collect tests ---

#[test]
fn collect_filters_and_maps_preserving_context() {
  // Given: a FlowWithContext that collects only positive values, doubling them
  let fwc: FlowWithContext<i32, i32, i32, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, i32)| v));
  let collected = fwc.collect(|v: i32| if v > 0 { Some(v * 2) } else { None });

  // When: elements are pushed through
  let values = Source::from(vec![(1_i32, 5), (2, -3), (3, 10)]).via(collected.as_flow()).collect_values().unwrap();

  // Then: only Some results pass, with transformation applied
  assert_eq!(values, vec![(1, 10), (3, 20)]);
}

#[test]
fn collect_drops_all_when_all_none() {
  // Given: a collect function that always returns None
  let fwc: FlowWithContext<i32, i32, i32, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, i32)| v));
  let collected = fwc.collect(|_: i32| -> Option<i32> { None });

  // When: elements are pushed through
  let values = Source::from(vec![(1_i32, 5), (2, 10)]).via(collected.as_flow()).collect_values().unwrap();

  // Then: no elements pass
  assert!(values.is_empty());
}

// --- map_async tests ---

#[test]
fn map_async_transforms_preserving_context() {
  // Given: a FlowWithContext with an async map that doubles the value
  let fwc: FlowWithContext<i32, u32, u32, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, u32)| v));
  let mapped = fwc.map_async(1, |v: u32| async move { v * 2 }).expect("map_async");

  // When: elements are pushed through
  let values = Source::from(vec![(1_i32, 5_u32), (2, 3)]).via(mapped.as_flow()).collect_values().unwrap();

  // Then: values are transformed, contexts preserved
  assert_eq!(values, vec![(1, 10_u32), (2, 6)]);
}

// --- grouped tests ---

#[test]
fn grouped_collects_elements_with_last_context() {
  // Given: a FlowWithContext that groups elements into batches of 2
  let fwc: FlowWithContext<i32, u32, u32, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, u32)| v));
  let grouped = fwc.grouped(2).expect("grouped");

  // When: 5 elements are pushed through
  let values = Source::from(vec![(10_i32, 1_u32), (20, 2), (30, 3), (40, 4), (50, 5)])
    .via(grouped.as_flow())
    .collect_values()
    .unwrap();

  // Then: elements are grouped, each group's context is the last element's context
  assert_eq!(values, vec![(20, vec![1_u32, 2]), (40, vec![3, 4]), (50, vec![5])]);
}

#[test]
fn grouped_single_element_per_group() {
  // Given: group size of 1
  let fwc: FlowWithContext<i32, u32, u32, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, u32)| v));
  let grouped = fwc.grouped(1).expect("grouped");

  // When: elements are pushed through
  let values = Source::from(vec![(1_i32, 10_u32), (2, 20)]).via(grouped.as_flow()).collect_values().unwrap();

  // Then: each element is its own group, context is preserved
  assert_eq!(values, vec![(1, vec![10_u32]), (2, vec![20])]);
}

// --- sliding tests ---

#[test]
fn sliding_creates_windows_with_last_context() {
  // Given: a FlowWithContext with sliding window of size 3
  let fwc: FlowWithContext<i32, u32, u32, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, u32)| v));
  let sliding = fwc.sliding(3).expect("sliding");

  // When: elements are pushed through
  let values =
    Source::from(vec![(10_i32, 1_u32), (20, 2), (30, 3), (40, 4)]).via(sliding.as_flow()).collect_values().unwrap();

  // Then: sliding windows are emitted, each window's context is the last element's context
  assert_eq!(values, vec![(30, vec![1_u32, 2, 3]), (40, vec![2, 3, 4]),]);
}

#[test]
fn sliding_window_size_2() {
  // Given: a FlowWithContext with sliding window of size 2
  let fwc: FlowWithContext<i32, u32, u32, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, u32)| v));
  let sliding = fwc.sliding(2).expect("sliding");

  // When: 3 elements are pushed through
  let values = Source::from(vec![(1_i32, 10_u32), (2, 20), (3, 30)]).via(sliding.as_flow()).collect_values().unwrap();

  // Then: 2 windows, each with context of the last element in the window
  assert_eq!(values, vec![(2, vec![10_u32, 20]), (3, vec![20, 30])]);
}
