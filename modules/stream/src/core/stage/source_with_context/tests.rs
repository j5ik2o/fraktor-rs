use alloc::{vec, vec::Vec};

use crate::core::{
  StreamNotUsed,
  stage::{FlowWithContext, Source, SourceWithContext, flow::Flow},
};

#[test]
fn should_create_from_source() {
  let source = Source::from(vec![(1_i32, "a"), (2, "b")]);
  let swc = SourceWithContext::from_source(source);
  let inner = swc.as_source();
  let values = inner.collect_values().unwrap();
  assert_eq!(values, vec![(1, "a"), (2, "b")]);
}

#[test]
fn should_map_output_preserving_context() {
  let source = Source::from(vec![(1_i32, "hello"), (2, "world")]);
  let swc = SourceWithContext::from_source(source);
  let mapped = swc.map(|s: &str| s.len());
  let values = mapped.as_source().collect_values().unwrap();
  assert_eq!(values, vec![(1, 5), (2, 5)]);
}

#[test]
fn should_filter_by_value_preserving_context() {
  let source = Source::from(vec![(1_i32, 10), (2, -5), (3, 20)]);
  let swc = SourceWithContext::from_source(source);
  let filtered = swc.filter(|v: &i32| *v > 0);
  let values = filtered.as_source().collect_values().unwrap();
  assert_eq!(values, vec![(1, 10), (3, 20)]);
}

#[test]
fn should_map_context() {
  let source = Source::from(vec![(1_i32, "a"), (2, "b")]);
  let swc = SourceWithContext::from_source(source);
  let mapped = swc.map_context(|ctx: i32| ctx * 10);
  let values = mapped.as_source().collect_values().unwrap();
  assert_eq!(values, vec![(10, "a"), (20, "b")]);
}

#[test]
fn should_compose_via() {
  let fwc: FlowWithContext<i32, &str, usize, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|(ctx, s): (i32, &str)| (ctx, s.len())));
  let swc = SourceWithContext::from_source(Source::from(vec![(1_i32, "hello"), (2, "hi")]));
  let composed = swc.via(fwc);
  let values = composed.as_source().collect_values().unwrap();
  assert_eq!(values, vec![(1, 5), (2, 2)]);
}

// --- map_concat tests ---

#[test]
fn should_map_concat_expanding_elements_with_same_context() {
  // Given: a SourceWithContext that expands each value into multiple values
  let source = Source::from(vec![(1_i32, "ab"), (2, "c")]);
  let swc = SourceWithContext::from_source(source);
  let expanded = swc.map_concat(|s: &str| s.chars().map(|c| c as u32).collect::<Vec<_>>());

  // When: elements are collected
  let values = expanded.as_source().collect_values().unwrap();

  // Then: each expanded element gets the same context as the original
  assert_eq!(values, vec![(1, 97), (1, 98), (2, 99)]);
}

#[test]
fn should_map_concat_dropping_empty_expansions() {
  // Given: a SourceWithContext where some elements expand to empty
  let source = Source::from(vec![(1_i32, 5), (2, -1), (3, 3)]);
  let swc = SourceWithContext::from_source(source);
  let expanded = swc.map_concat(|v: i32| if v > 0 { vec![v, v * 10] } else { vec![] });

  // When: elements are collected
  let values = expanded.as_source().collect_values().unwrap();

  // Then: empty expansions are dropped
  assert_eq!(values, vec![(1, 5), (1, 50), (3, 3), (3, 30)]);
}

// --- filter_not tests ---

#[test]
fn should_filter_not_passing_false_predicate_elements() {
  // Given: a SourceWithContext that rejects positive values
  let source = Source::from(vec![(1_i32, 10), (2, -5), (3, 0), (4, 20)]);
  let swc = SourceWithContext::from_source(source);
  let filtered = swc.filter_not(|v: &i32| *v > 0);

  // When: elements are collected
  let values = filtered.as_source().collect_values().unwrap();

  // Then: only elements where predicate is false pass
  assert_eq!(values, vec![(2, -5), (3, 0)]);
}

#[test]
fn should_filter_not_passing_all_when_predicate_always_false() {
  // Given: a predicate that is always false
  let source = Source::from(vec![(1_i32, 10), (2, 20)]);
  let swc = SourceWithContext::from_source(source);
  let filtered = swc.filter_not(|_: &i32| false);

  // When: elements are collected
  let values = filtered.as_source().collect_values().unwrap();

  // Then: all elements pass
  assert_eq!(values, vec![(1, 10), (2, 20)]);
}

// --- collect tests ---

#[test]
fn should_collect_filtering_and_mapping_with_context() {
  // Given: a SourceWithContext that collects positive values doubled
  let source = Source::from(vec![(1_i32, 5), (2, -3), (3, 10)]);
  let swc = SourceWithContext::from_source(source);
  let collected = swc.collect(|v: i32| if v > 0 { Some(v * 2) } else { None });

  // When: elements are collected
  let values = collected.as_source().collect_values().unwrap();

  // Then: only Some results pass with transformation applied
  assert_eq!(values, vec![(1, 10), (3, 20)]);
}

#[test]
fn should_collect_dropping_all_when_all_none() {
  // Given: a collect function that always returns None
  let source = Source::from(vec![(1_i32, 5), (2, 10)]);
  let swc = SourceWithContext::from_source(source);
  let collected = swc.collect(|_: i32| -> Option<i32> { None });

  // When: elements are collected
  let values = collected.as_source().collect_values().unwrap();

  // Then: no elements pass
  assert!(values.is_empty());
}

// --- map_async tests ---

#[test]
fn should_map_async_transforming_with_context() {
  // Given: a SourceWithContext with async map that doubles the value
  let source = Source::from(vec![(1_i32, 5_u32), (2, 3)]);
  let swc = SourceWithContext::from_source(source);
  let mapped = swc.map_async(1, |v: u32| async move { v * 2 }).expect("map_async");

  // When: elements are collected
  let values = mapped.as_source().collect_values().unwrap();

  // Then: values are transformed, contexts preserved
  assert_eq!(values, vec![(1, 10_u32), (2, 6)]);
}

// --- grouped tests ---

#[test]
fn should_grouped_collecting_elements_with_last_context() {
  // Given: a SourceWithContext that groups elements into batches of 2
  let source = Source::from(vec![(10_i32, 1_u32), (20, 2), (30, 3), (40, 4), (50, 5)]);
  let swc = SourceWithContext::from_source(source);
  let grouped = swc.grouped(2).expect("grouped");

  // When: elements are collected
  let values = grouped.as_source().collect_values().unwrap();

  // Then: elements are grouped, each group's context is the last element's context
  assert_eq!(values, vec![(20, vec![1_u32, 2]), (40, vec![3, 4]), (50, vec![5])]);
}

#[test]
fn should_grouped_single_element_per_group() {
  // Given: group size of 1
  let source = Source::from(vec![(1_i32, 10_u32), (2, 20)]);
  let swc = SourceWithContext::from_source(source);
  let grouped = swc.grouped(1).expect("grouped");

  // When: elements are collected
  let values = grouped.as_source().collect_values().unwrap();

  // Then: each element is its own group, context preserved
  assert_eq!(values, vec![(1, vec![10_u32]), (2, vec![20])]);
}

// --- sliding tests ---

#[test]
fn should_sliding_creating_windows_with_last_context() {
  // Given: a SourceWithContext with sliding window of size 3
  let source = Source::from(vec![(10_i32, 1_u32), (20, 2), (30, 3), (40, 4)]);
  let swc = SourceWithContext::from_source(source);
  let sliding = swc.sliding(3).expect("sliding");

  // When: elements are collected
  let values = sliding.as_source().collect_values().unwrap();

  // Then: sliding windows, each window's context is the last element's context
  assert_eq!(values, vec![(30, vec![1_u32, 2, 3]), (40, vec![2, 3, 4]),]);
}

#[test]
fn should_sliding_window_size_2() {
  // Given: a SourceWithContext with sliding window of size 2
  let source = Source::from(vec![(1_i32, 10_u32), (2, 20), (3, 30)]);
  let swc = SourceWithContext::from_source(source);
  let sliding = swc.sliding(2).expect("sliding");

  // When: elements are collected
  let values = sliding.as_source().collect_values().unwrap();

  // Then: 2 windows with last element's context
  assert_eq!(values, vec![(2, vec![10_u32, 20]), (3, vec![20, 30])]);
}
