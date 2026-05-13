mod support;
use core::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};
use std::sync::{Arc, Mutex};

use fraktor_stream_core_kernel_rs::{
  StreamError, ThrottleMode,
  dsl::{Flow, FlowWithContext, Sink, Source, SourceWithContext},
  materialization::{KeepBoth, KeepLeft, KeepRight, StreamNotUsed},
};
use support::RunWithCollectSink;

#[derive(Default)]
struct YieldThenOutputFuture<T> {
  value:       Option<T>,
  poll_count:  u8,
  ready_after: u8,
}

impl<T> YieldThenOutputFuture<T> {
  fn new(value: T, ready_after: u8) -> Self {
    Self { value: Some(value), poll_count: 0, ready_after }
  }
}

impl<T: Unpin> Future for YieldThenOutputFuture<T> {
  type Output = T;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let this = self.get_mut();
    if this.poll_count < this.ready_after {
      this.poll_count = this.poll_count.saturating_add(1);
      cx.waker().wake_by_ref();
      Poll::Pending
    } else {
      Poll::Ready(this.value.take().expect("future value"))
    }
  }
}

#[test]
fn should_create_from_source() {
  let source = Source::from(vec![(1_i32, "a"), (2, "b")]);
  let swc = SourceWithContext::from_source(source);
  let inner = swc.into_source();
  let values = inner.run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![(1, "a"), (2, "b")]);
}

#[test]
fn should_map_output_preserving_context() {
  let source = Source::from(vec![(1_i32, "hello"), (2, "world")]);
  let swc = SourceWithContext::from_source(source);
  let mapped = swc.map(|s: &str| s.len());
  let values = mapped.into_source().run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![(1, 5), (2, 5)]);
}

#[test]
fn should_filter_by_value_preserving_context() {
  let source = Source::from(vec![(1_i32, 10), (2, -5), (3, 20)]);
  let swc = SourceWithContext::from_source(source);
  let filtered = swc.filter(|v: &i32| *v > 0);
  let values = filtered.into_source().run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![(1, 10), (3, 20)]);
}

#[test]
fn should_map_context() {
  let source = Source::from(vec![(1_i32, "a"), (2, "b")]);
  let swc = SourceWithContext::from_source(source);
  let mapped = swc.map_context(|ctx: i32| ctx * 10);
  let values = mapped.into_source().run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![(10, "a"), (20, "b")]);
}

#[test]
fn should_compose_via() {
  let fwc: FlowWithContext<i32, &str, usize, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|(ctx, s): (i32, &str)| (ctx, s.len())));
  let swc = SourceWithContext::from_source(Source::from(vec![(1_i32, "hello"), (2, "hi")]));
  let composed = swc.via(fwc);
  let values = composed.into_source().run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![(1, 5), (2, 2)]);
}

#[test]
fn should_map_concat_expanding_elements_with_same_context() {
  let source = Source::from(vec![(1_i32, "ab"), (2, "c")]);
  let swc = SourceWithContext::from_source(source);
  let expanded = swc.map_concat(|s: &str| s.chars().map(|c| c as u32).collect::<Vec<_>>());

  let values = expanded.into_source().run_with_collect_sink().expect("run_with_collect_sink");

  assert_eq!(values, vec![(1, 97), (1, 98), (2, 99)]);
}

#[test]
fn should_map_concat_dropping_empty_expansions() {
  let source = Source::from(vec![(1_i32, 5), (2, -1), (3, 3)]);
  let swc = SourceWithContext::from_source(source);
  let expanded = swc.map_concat(|v: i32| if v > 0 { vec![v, v * 10] } else { vec![] });

  let values = expanded.into_source().run_with_collect_sink().expect("run_with_collect_sink");

  assert_eq!(values, vec![(1, 5), (1, 50), (3, 3), (3, 30)]);
}

#[test]
fn should_filter_not_passing_false_predicate_elements() {
  let source = Source::from(vec![(1_i32, 10), (2, -5), (3, 0), (4, 20)]);
  let swc = SourceWithContext::from_source(source);
  let filtered = swc.filter_not(|v: &i32| *v > 0);

  let values = filtered.into_source().run_with_collect_sink().expect("run_with_collect_sink");

  assert_eq!(values, vec![(2, -5), (3, 0)]);
}

#[test]
fn should_filter_not_passing_all_when_predicate_always_false() {
  let source = Source::from(vec![(1_i32, 10), (2, 20)]);
  let swc = SourceWithContext::from_source(source);
  let filtered = swc.filter_not(|_: &i32| false);

  let values = filtered.into_source().run_with_collect_sink().expect("run_with_collect_sink");

  assert_eq!(values, vec![(1, 10), (2, 20)]);
}

#[test]
fn should_collect_filtering_and_mapping_with_context() {
  let source = Source::from(vec![(1_i32, 5), (2, -3), (3, 10)]);
  let swc = SourceWithContext::from_source(source);
  let collected = swc.collect(|v: i32| if v > 0 { Some(v * 2) } else { None });

  let values = collected.into_source().run_with_collect_sink().expect("run_with_collect_sink");

  assert_eq!(values, vec![(1, 10), (3, 20)]);
}

#[test]
fn should_collect_dropping_all_when_all_none() {
  let source = Source::from(vec![(1_i32, 5), (2, 10)]);
  let swc = SourceWithContext::from_source(source);
  let collected = swc.collect(|_: i32| -> Option<i32> { None });

  let values = collected.into_source().run_with_collect_sink().expect("run_with_collect_sink");

  assert!(values.is_empty());
}

#[test]
fn should_map_async_transforming_with_context() {
  let source = Source::from(vec![(1_i32, 5_u32), (2, 3)]);
  let swc = SourceWithContext::from_source(source);
  let mapped = swc.map_async(1, |v: u32| async move { v * 2 }).expect("map_async");

  let values = mapped.into_source().run_with_collect_sink().expect("run_with_collect_sink");

  assert_eq!(values, vec![(1, 10_u32), (2, 6)]);
}

#[test]
fn should_grouped_collecting_elements_with_last_context() {
  let source = Source::from(vec![(10_i32, 1_u32), (20, 2), (30, 3), (40, 4), (50, 5)]);
  let swc = SourceWithContext::from_source(source);
  let grouped = swc.grouped(2).expect("grouped");

  let values = grouped.into_source().run_with_collect_sink().expect("run_with_collect_sink");

  assert_eq!(values, vec![(20, vec![1_u32, 2]), (40, vec![3, 4]), (50, vec![5])]);
}

#[test]
fn should_grouped_single_element_per_group() {
  let source = Source::from(vec![(1_i32, 10_u32), (2, 20)]);
  let swc = SourceWithContext::from_source(source);
  let grouped = swc.grouped(1).expect("grouped");

  let values = grouped.into_source().run_with_collect_sink().expect("run_with_collect_sink");

  assert_eq!(values, vec![(1, vec![10_u32]), (2, vec![20])]);
}

#[test]
fn should_sliding_creating_windows_with_last_context() {
  let source = Source::from(vec![(10_i32, 1_u32), (20, 2), (30, 3), (40, 4)]);
  let swc = SourceWithContext::from_source(source);
  let sliding = swc.sliding(3).expect("sliding");

  let values = sliding.into_source().run_with_collect_sink().expect("run_with_collect_sink");

  assert_eq!(values, vec![(30, vec![1_u32, 2, 3]), (40, vec![2, 3, 4])]);
}

#[test]
fn should_sliding_window_size_2() {
  let source = Source::from(vec![(1_i32, 10_u32), (2, 20), (3, 30)]);
  let swc = SourceWithContext::from_source(source);
  let sliding = swc.sliding(2).expect("sliding");

  let values = sliding.into_source().run_with_collect_sink().expect("run_with_collect_sink");

  assert_eq!(values, vec![(2, vec![10_u32, 20]), (3, vec![20, 30])]);
}

#[test]
fn should_via_mat_combine_source_and_flow_materialized_values() {
  let source = Source::from(vec![(1_i32, 10_u32)]).map_materialized_value(|_| 5_u32);
  let swc = SourceWithContext::from_source(source);
  let flow: FlowWithContext<i32, u32, u32, u32> = FlowWithContext::from_flow(
    Flow::new().map(|(ctx, value): (i32, u32)| (ctx, value + 1)).map_materialized_value(|_| 9_u32),
  );

  let graph = swc.via_mat(flow, KeepBoth).into_source().into_mat(Sink::<(i32, u32), _>::ignore(), KeepLeft);

  assert_eq!(graph.materialized(), &(5_u32, 9_u32));
}

#[test]
fn should_via_mat_keep_flow_materialized_value_when_requested() {
  let source = Source::from(vec![(1_i32, 10_u32)]).map_materialized_value(|_| 5_u32);
  let swc = SourceWithContext::from_source(source);
  let flow: FlowWithContext<i32, u32, u32, u32> = FlowWithContext::from_flow(
    Flow::new().map(|(ctx, value): (i32, u32)| (ctx, value + 1)).map_materialized_value(|_| 9_u32),
  );

  let graph = swc.via_mat(flow, KeepRight).into_source().into_mat(Sink::<(i32, u32), _>::ignore(), KeepLeft);

  assert_eq!(graph.materialized(), &9_u32);
}

#[test]
fn should_also_to_send_values_to_side_sink_and_preserve_main_path() {
  let seen = Arc::new(Mutex::new(Vec::new()));
  let seen_for_sink = Arc::clone(&seen);
  let source = Source::from(vec![(10_i32, 1_u32), (20, 2)]);
  let swc = SourceWithContext::from_source(source).also_to(Sink::foreach(move |value: u32| {
    seen_for_sink.lock().expect("side sink lock").push(value);
  }));

  let values = swc.into_source().run_with_collect_sink().expect("run_with_collect_sink");

  assert_eq!(values, vec![(10_i32, 1_u32), (20, 2)]);
  assert_eq!(*seen.lock().expect("seen lock"), vec![1_u32, 2]);
}

#[test]
fn should_also_to_context_send_only_contexts_to_side_sink() {
  let seen = Arc::new(Mutex::new(Vec::new()));
  let seen_for_sink = Arc::clone(&seen);
  let source = Source::from(vec![(10_i32, 1_u32), (20, 2)]);
  let swc = SourceWithContext::from_source(source).also_to_context(Sink::foreach(move |ctx: i32| {
    seen_for_sink.lock().expect("context sink lock").push(ctx);
  }));

  let values = swc.into_source().run_with_collect_sink().expect("run_with_collect_sink");

  assert_eq!(values, vec![(10_i32, 1_u32), (20, 2)]);
  assert_eq!(*seen.lock().expect("seen lock"), vec![10_i32, 20]);
}

#[test]
fn should_wire_tap_preserve_main_path_and_emit_values() {
  let seen = Arc::new(Mutex::new(Vec::new()));
  let seen_for_sink = Arc::clone(&seen);
  let source = Source::from(vec![(10_i32, 1_u32), (20, 2)]);
  let swc = SourceWithContext::from_source(source).wire_tap(Sink::foreach(move |value: u32| {
    seen_for_sink.lock().expect("tap sink lock").push(value);
  }));

  let values = swc.into_source().run_with_collect_sink().expect("run_with_collect_sink");

  assert_eq!(values, vec![(10_i32, 1_u32), (20, 2)]);
  assert_eq!(*seen.lock().expect("seen lock"), vec![1_u32, 2]);
}

#[test]
fn should_wire_tap_context_preserve_main_path_and_emit_contexts() {
  let seen = Arc::new(Mutex::new(Vec::new()));
  let seen_for_sink = Arc::clone(&seen);
  let source = Source::from(vec![(10_i32, 1_u32), (20, 2)]);
  let swc = SourceWithContext::from_source(source).wire_tap_context(Sink::foreach(move |ctx: i32| {
    seen_for_sink.lock().expect("context tap lock").push(ctx);
  }));

  let values = swc.into_source().run_with_collect_sink().expect("run_with_collect_sink");

  assert_eq!(values, vec![(10_i32, 1_u32), (20, 2)]);
  assert_eq!(*seen.lock().expect("seen lock"), vec![10_i32, 20]);
}

#[test]
fn should_map_async_partitioned_preserving_context_and_input_order() {
  let source = Source::from(vec![(100_i32, 1_u32), (200, 2)]);
  let swc = SourceWithContext::from_source(source);
  let mapped = swc
    .map_async_partitioned(
      2,
      |value: &u32| (*value as usize) % 2,
      |value: u32, partition: usize| {
        let ready_after = if partition == 1 { 2 } else { 0 };
        YieldThenOutputFuture::new(value + 10, ready_after)
      },
    )
    .expect("map_async_partitioned");

  let values = mapped.into_source().run_with_collect_sink().expect("run_with_collect_sink");

  assert_eq!(values, vec![(100_i32, 11_u32), (200, 12)]);
}

#[test]
fn should_map_async_partitioned_unordered_emitting_completion_order_with_context() {
  let source = Source::from(vec![(100_i32, 1_u32), (200, 2)]);
  let swc = SourceWithContext::from_source(source);
  let mapped = swc
    .map_async_partitioned_unordered(
      2,
      |value: &u32| (*value as usize) % 2,
      |value: u32, partition: usize| {
        let ready_after = if partition == 1 { 16 } else { 0 };
        YieldThenOutputFuture::new(value + 10, ready_after)
      },
    )
    .expect("map_async_partitioned_unordered");

  let values = mapped.into_source().run_with_collect_sink().expect("run_with_collect_sink");

  assert_eq!(values, vec![(200_i32, 12_u32), (100, 11)]);
}

#[test]
fn should_map_error_passing_normal_elements_with_context() {
  let source = Source::from(vec![(1_i32, 10_u32), (2, 20)]);
  let swc = SourceWithContext::from_source(source).map_error(|_| StreamError::WouldBlock);

  let values = swc.into_source().run_with_collect_sink().expect("run_with_collect_sink");

  assert_eq!(values, vec![(1, 10_u32), (2, 20)]);
}

#[test]
fn should_map_error_transforming_upstream_failure() {
  let source = Source::<(i32, u32), _>::failed(StreamError::Failed);
  let swc = SourceWithContext::from_source(source).map_error(|_| StreamError::WouldBlock);

  let result = swc.into_source().run_with_collect_sink();

  assert_eq!(result, Err(StreamError::WouldBlock));
}

#[test]
fn should_throttle_passing_elements_with_context() {
  let source = Source::from(vec![(1_i32, 10_u32), (2, 20)]);
  let swc = SourceWithContext::from_source(source).throttle(2, ThrottleMode::Shaping).expect("throttle");

  let values = swc.into_source().run_with_collect_sink().expect("run_with_collect_sink");

  assert_eq!(values, vec![(1, 10_u32), (2, 20)]);
}

#[test]
fn should_throttle_enforcing_mode_preserving_context() {
  let source = Source::from(vec![(1_i32, 10_u32)]);
  let swc = SourceWithContext::from_source(source).throttle(2, ThrottleMode::Enforcing).expect("throttle");

  let values = swc.into_source().run_with_collect_sink().expect("run_with_collect_sink");

  assert_eq!(values, vec![(1, 10_u32)]);
}

#[test]
fn should_throttle_rejecting_zero_capacity() {
  let source = Source::from(vec![(1_i32, 10_u32)]);
  let swc = SourceWithContext::from_source(source);

  let result = swc.throttle(0, ThrottleMode::Shaping);

  assert!(result.is_err());
}
