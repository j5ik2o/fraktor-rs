use alloc::{boxed::Box, collections::VecDeque};
use core::{future::Future, pin::Pin, task::Poll};

use fraktor_utils_rs::core::collections::queue::OverflowPolicy;

use crate::core::{
  Completion, DynValue, FlowLogic, KeepBoth, KeepLeft, KeepRight, RestartSettings, SourceLogic, StreamDslError,
  StreamError, StreamNotUsed,
  operator::{DefaultOperatorCatalog, OperatorCatalog, OperatorKey},
  stage::{Flow, FlowMonitor, Sink, Source, StageKind},
};

struct SequenceSourceLogic {
  values: VecDeque<u32>,
}

impl SequenceSourceLogic {
  fn new(values: &[u32]) -> Self {
    let mut queue = VecDeque::with_capacity(values.len());
    queue.extend(values.iter().copied());
    Self { values: queue }
  }
}

impl SourceLogic for SequenceSourceLogic {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    Ok(self.values.pop_front().map(|value| Box::new(value) as DynValue))
  }
}

#[test]
fn broadcast_duplicates_each_element() {
  let values = Source::single(7_u32).via(Flow::new().broadcast(2)).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32, 7_u32]);
}

#[test]
#[should_panic(expected = "fan_out must be greater than zero")]
fn broadcast_rejects_zero_fan_out() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let _ = flow.broadcast(0);
}

#[test]
fn balance_keeps_single_path_behavior() {
  let values = Source::single(7_u32).via(Flow::new().balance(1)).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
#[should_panic(expected = "fan_out must be greater than zero")]
fn balance_rejects_zero_fan_out() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let _ = flow.balance(0);
}

#[test]
fn merge_keeps_single_path_behavior() {
  let values = Source::single(7_u32).via(Flow::new().merge(1)).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
#[should_panic(expected = "fan_in must be greater than zero")]
fn merge_rejects_zero_fan_in() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let _ = flow.merge(0);
}

#[test]
fn zip_wraps_value_when_single_path() {
  let values = Source::single(7_u32).via(Flow::new().zip(1)).collect_values().expect("collect_values");
  assert_eq!(values, vec![vec![7_u32]]);
}

#[test]
#[should_panic(expected = "fan_in must be greater than zero")]
fn zip_rejects_zero_fan_in() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let _ = flow.zip(0);
}

#[test]
fn concat_keeps_single_path_behavior() {
  let values = Source::single(7_u32).via(Flow::new().concat(1)).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
#[should_panic(expected = "fan_in must be greater than zero")]
fn concat_rejects_zero_fan_in() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let _ = flow.concat(0);
}

#[test]
fn partition_keeps_single_path_behavior() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .via(Flow::new().partition(|value| *value % 2 == 0))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32, 4_u32]);
}

#[test]
fn unzip_emits_tuple_components() {
  let values = Source::single((7_u32, 8_u32)).via(Flow::new().unzip()).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32, 8_u32]);
}

#[test]
fn unzip_with_emits_mapped_tuple_components() {
  let values = Source::single(7_u32)
    .via(Flow::new().unzip_with(|value: u32| (value, value.saturating_add(1))))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![7_u32, 8_u32]);
}

#[test]
fn interleave_keeps_single_path_behavior() {
  let values = Source::single(7_u32).via(Flow::new().interleave(1)).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
#[should_panic(expected = "fan_in must be greater than zero")]
fn interleave_rejects_zero_fan_in() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let _ = flow.interleave(0);
}

#[test]
fn prepend_keeps_single_path_behavior() {
  let values = Source::single(7_u32).via(Flow::new().prepend(1)).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
#[should_panic(expected = "fan_in must be greater than zero")]
fn prepend_rejects_zero_fan_in() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let _ = flow.prepend(0);
}

#[test]
fn zip_all_wraps_value_when_single_path() {
  let values = Source::single(7_u32).via(Flow::new().zip_all(1, 0_u32)).collect_values().expect("collect_values");
  assert_eq!(values, vec![vec![7_u32]]);
}

#[test]
#[should_panic(expected = "fan_in must be greater than zero")]
fn zip_all_rejects_zero_fan_in() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let _ = flow.zip_all(0, 0_u32);
}

#[test]
fn flat_map_merge_keeps_single_path_behavior() {
  let values = Source::single(7_u32)
    .via(Flow::new().flat_map_merge(2, Source::single).expect("flat_map_merge"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn flat_map_merge_rejects_zero_breadth() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let result = flow.flat_map_merge(0, Source::single);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "breadth", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn buffer_keeps_single_path_behavior() {
  let values = Source::single(7_u32)
    .via(Flow::new().buffer(2, OverflowPolicy::Block).expect("buffer"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn buffer_rejects_zero_capacity() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let result = flow.buffer(0, OverflowPolicy::Block);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "capacity", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn async_boundary_keeps_single_path_behavior() {
  let values = Source::single(7_u32).via(Flow::new().async_boundary()).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn throttle_keeps_single_path_behavior() {
  let values =
    Source::single(7_u32).via(Flow::new().throttle(2).expect("throttle")).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn throttle_rejects_zero_capacity() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let result = flow.throttle(0);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "capacity", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn delay_keeps_single_path_behavior() {
  let values =
    Source::single(7_u32).via(Flow::new().delay(2).expect("delay")).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn delay_rejects_zero_ticks() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let result = flow.delay(0);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "ticks", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn initial_delay_keeps_single_path_behavior() {
  let values = Source::single(7_u32)
    .via(Flow::new().initial_delay(2).expect("initial_delay"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn initial_delay_rejects_zero_ticks() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let result = flow.initial_delay(0);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "ticks", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn take_within_limits_output_window() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::new().take_within(1).expect("take_within"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32]);
}

#[test]
fn take_within_rejects_zero_ticks() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let result = flow.take_within(0);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "ticks", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn batch_emits_fixed_size_chunks() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4, 5]))
    .via(Flow::new().batch(2).expect("batch"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![vec![1_u32, 2_u32], vec![3_u32, 4_u32], vec![5_u32]]);
}

#[test]
fn batch_rejects_zero_size() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let result = flow.batch(0);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "size", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn map_async_keeps_single_path_behavior() {
  let values = Source::single(7_u32)
    .via(Flow::new().map_async(2, |value: u32| async move { value.saturating_add(1) }).expect("map_async"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![8_u32]);
}

#[test]
fn map_async_rejects_zero_parallelism() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let result = flow.map_async(0, |value| async move { value });
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "parallelism", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn map_async_preserves_order_with_parallelism() {
  let values = Source::from_array([1_u32, 2_u32, 3_u32])
    .via(
      Flow::new()
        .map_async(2, |value: u32| match value {
          | 1 => YieldThenOutputFuture::new_with_poll_count(value.saturating_add(1), 2),
          | 2 => YieldThenOutputFuture::new(value.saturating_add(1)),
          | _ => YieldThenOutputFuture::new_with_poll_count(value.saturating_add(1), 3),
        })
        .expect("map_async"),
    )
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![2_u32, 3_u32, 4_u32]);
}

#[test]
fn map_async_logic_keeps_order_and_tracks_pending_output() {
  let mut logic = super::MapAsyncLogic::<u32, u32, _, YieldThenOutputFuture<u32>> {
    func:        |value: u32| YieldThenOutputFuture::new(value.saturating_add(1)),
    parallelism: 2,
    pending:     VecDeque::new(),
    _pd:         core::marker::PhantomData,
  };

  assert!(logic.can_accept_input());
  let _ = logic.apply(Box::new(1_u32)).expect("apply");
  assert!(logic.has_pending_output());

  assert!(logic.can_accept_input());
  let _ = logic.apply(Box::new(2_u32)).expect("apply");
  assert!(!logic.can_accept_input());

  let outputs = logic.drain_pending().expect("drain");
  assert_eq!(outputs.len(), 0);

  let outputs = logic.drain_pending().expect("drain");
  assert_eq!(outputs.len(), 2);
  let output_values: Vec<u32> =
    outputs.into_iter().map(|value: DynValue| *value.downcast::<u32>().expect("u32")).collect();
  assert_eq!(output_values, vec![2_u32, 3_u32]);
  assert!(!logic.has_pending_output());
  assert!(logic.can_accept_input());
}

#[derive(Default)]
struct YieldThenOutputFuture<T> {
  value:       Option<T>,
  poll_count:  u8,
  ready_after: u8,
}

impl<T> YieldThenOutputFuture<T> {
  fn new(value: T) -> Self {
    Self { value: Some(value), poll_count: 0, ready_after: 1 }
  }

  fn new_with_poll_count(value: T, poll_count: u8) -> Self {
    Self { value: Some(value), poll_count: 0, ready_after: poll_count }
  }
}

impl<T> Future for YieldThenOutputFuture<T> {
  type Output = T;

  fn poll(self: Pin<&mut Self>, _cx: &mut core::task::Context<'_>) -> Poll<Self::Output> {
    let this = unsafe { self.get_unchecked_mut() };
    if this.poll_count < this.ready_after {
      this.poll_count = this.poll_count.saturating_add(1);
      Poll::Pending
    } else {
      Poll::Ready(this.value.take().expect("future value"))
    }
  }
}

#[test]
fn filter_keeps_matching_elements() {
  let values =
    Source::single(7_u32).via(Flow::new().filter(|value| *value % 2 == 1)).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn filter_discards_non_matching_elements() {
  let values =
    Source::single(8_u32).via(Flow::new().filter(|value| *value % 2 == 1)).collect_values().expect("collect_values");
  assert_eq!(values, Vec::<u32>::new());
}

#[test]
fn filter_not_discards_matching_elements() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .via(Flow::new().filter_not(|value| *value % 2 == 0))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 3_u32]);
}

#[test]
fn flatten_optional_emits_present_value() {
  let values =
    Source::single(Some(7_u32)).via(Flow::new().flatten_optional()).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn flatten_optional_skips_none() {
  let values =
    Source::single(None::<u32>).via(Flow::new().flatten_optional()).collect_values().expect("collect_values");
  assert_eq!(values, Vec::<u32>::new());
}

#[test]
fn map_concat_expands_each_element() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::new().map_concat(|value: u32| [value, value.saturating_add(10)]))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 11_u32, 2_u32, 12_u32, 3_u32, 13_u32]);
}

#[test]
fn map_option_emits_only_present_elements() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .via(Flow::new().map_option(|value| if value % 2 == 0 { Some(value) } else { None }))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![2_u32, 4_u32]);
}

#[test]
fn stateful_map_emits_stateful_results() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::new().stateful_map(|| {
      let mut sum = 0_u32;
      move |value| {
        sum = sum.saturating_add(value);
        sum
      }
    }))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 3_u32, 6_u32]);
}

#[test]
fn stateful_map_concat_expands_with_stateful_mapper() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::new().stateful_map_concat(|| {
      let mut sum = 0_u32;
      move |value| {
        sum = sum.saturating_add(value);
        [sum, sum.saturating_add(100)]
      }
    }))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 101_u32, 3_u32, 103_u32, 6_u32, 106_u32]);
}

#[test]
fn drop_skips_first_elements() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .via(Flow::new().drop(2))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![3_u32, 4_u32]);
}

#[test]
fn take_limits_emitted_elements() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .via(Flow::new().take(2))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32]);
}

#[test]
fn drop_while_skips_matching_prefix() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .via(Flow::new().drop_while(|value| *value < 3))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![3_u32, 4_u32]);
}

#[test]
fn take_while_keeps_matching_prefix() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .via(Flow::new().take_while(|value| *value < 3))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32]);
}

#[test]
fn take_until_includes_first_matching_element() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .via(Flow::new().take_until(|value| *value >= 3))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32]);
}

#[test]
fn grouped_emits_fixed_size_chunks() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4, 5]))
    .via(Flow::new().grouped(2).expect("grouped"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![vec![1_u32, 2_u32], vec![3_u32, 4_u32], vec![5_u32]]);
}

#[test]
fn grouped_rejects_zero_size() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let result = flow.grouped(0);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "size", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn sliding_emits_overlapping_windows() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3, 4]))
    .via(Flow::new().sliding(3).expect("sliding"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![vec![1_u32, 2_u32, 3_u32], vec![2_u32, 3_u32, 4_u32]]);
}

#[test]
fn sliding_rejects_zero_size() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let result = flow.sliding(0);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "size", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn scan_emits_initial_and_running_accumulation() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::new().scan(0_u32, |acc, value| acc + value))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![0_u32, 1_u32, 3_u32, 6_u32]);
}

#[test]
fn intersperse_injects_markers() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::new().intersperse(10_u32, 99_u32, 11_u32))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![10_u32, 1_u32, 99_u32, 2_u32, 99_u32, 3_u32, 11_u32]);
}

#[test]
fn intersperse_on_empty_stream_emits_start_and_end() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[]))
    .via(Flow::new().intersperse(10_u32, 99_u32, 11_u32))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![10_u32, 11_u32]);
}

#[test]
fn zip_with_index_pairs_each_element_with_index() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[7, 8, 9]))
    .via(Flow::new().zip_with_index())
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![(7_u32, 0_u64), (8_u32, 1_u64), (9_u32, 2_u64)]);
}

#[test]
fn group_by_keeps_single_path_behavior() {
  let values = Source::single(7_u32)
    .via(Flow::new().group_by(4, |value: &u32| value % 2).expect("group_by").merge_substreams())
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn group_by_rejects_zero_max_substreams() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new();
  let result = flow.group_by(0, |value: &u32| *value);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "max_substreams", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn split_when_emits_single_segment_for_single_element() {
  let values = Source::single(7_u32)
    .via(Flow::<u32, u32, StreamNotUsed>::new().split_when(|_| false).into_flow())
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![vec![7_u32]]);
}

#[test]
fn split_after_emits_single_segment_for_single_element() {
  let values = Source::single(7_u32)
    .via(Flow::<u32, u32, StreamNotUsed>::new().split_after(|_| false).into_flow())
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![vec![7_u32]]);
}

#[test]
fn merge_substreams_flattens_single_segment() {
  let values = Source::single(7_u32)
    .via(Flow::<u32, u32, StreamNotUsed>::new().split_after(|_| true).merge_substreams())
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn concat_substreams_flattens_single_segment() {
  let values = Source::single(7_u32)
    .via(Flow::<u32, u32, StreamNotUsed>::new().split_after(|_| true).concat_substreams())
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn merge_substreams_with_parallelism_flattens_single_segment() {
  let values = Source::single(7_u32)
    .via(
      Flow::<u32, u32, StreamNotUsed>::new()
        .split_after(|_| true)
        .merge_substreams_with_parallelism(2)
        .expect("merge_substreams_with_parallelism"),
    )
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn merge_substreams_with_parallelism_rejects_zero_parallelism() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new().split_after(|_| true);
  let result = flow.merge_substreams_with_parallelism(0);
  assert!(matches!(
    result,
    Err(StreamDslError::InvalidArgument { name: "parallelism", value: 0, reason: "must be greater than zero" })
  ));
}

#[test]
fn map_error_maps_error_payload() {
  let values = Source::single(Err::<u32, StreamError>(StreamError::Failed))
    .via(Flow::new().map_error(|_| StreamError::WouldBlock))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![Err(StreamError::WouldBlock)]);
}

#[test]
fn on_error_continue_drops_error_payloads() {
  let values = Source::from_array([
    Ok::<u32, StreamError>(1_u32),
    Err::<u32, StreamError>(StreamError::Failed),
    Ok::<u32, StreamError>(2_u32),
  ])
  .via(Flow::new().on_error_continue())
  .collect_values()
  .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32]);
}

#[test]
fn on_error_resume_alias_drops_error_payloads() {
  let values = Source::from_array([
    Ok::<u32, StreamError>(1_u32),
    Err::<u32, StreamError>(StreamError::Failed),
    Ok::<u32, StreamError>(2_u32),
  ])
  .via(Flow::new().on_error_resume())
  .collect_values()
  .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32]);
}

#[test]
fn on_error_complete_stops_emitting_after_first_error_payload() {
  let values = Source::from_array([
    Ok::<u32, StreamError>(1_u32),
    Err::<u32, StreamError>(StreamError::Failed),
    Ok::<u32, StreamError>(2_u32),
  ])
  .via(Flow::new().on_error_complete())
  .collect_values()
  .expect("collect_values");
  assert_eq!(values, vec![1_u32]);
}

#[test]
fn recover_replaces_error_payload_with_fallback() {
  let values = Source::single(Err::<u32, StreamError>(StreamError::Failed))
    .via(Flow::new().recover(9_u32))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![9_u32]);
}

#[test]
fn recover_preserves_ok_values_and_replaces_error_payloads() {
  let values = Source::from_array([
    Ok::<u32, StreamError>(1_u32),
    Err::<u32, StreamError>(StreamError::Failed),
    Ok::<u32, StreamError>(2_u32),
  ])
  .via(Flow::new().recover(9_u32))
  .collect_values()
  .expect("collect_values");
  assert_eq!(values, vec![1_u32, 9_u32, 2_u32]);
}

#[test]
fn recover_with_alias_replaces_error_payload_with_fallback() {
  let values = Source::single(Err::<u32, StreamError>(StreamError::Failed))
    .via(Flow::new().recover_with(8_u32))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![8_u32]);
}

#[test]
fn recover_with_retries_fails_when_retry_budget_is_exhausted() {
  let result = Source::single(Err::<u32, StreamError>(StreamError::Failed))
    .via(Flow::new().recover_with_retries(0, 9_u32))
    .collect_values();
  assert_eq!(result, Err(StreamError::Failed));
}

#[test]
fn recover_with_retries_emits_fallback_until_budget_exhausts() {
  let values = Source::from_array([
    Err::<u32, StreamError>(StreamError::Failed),
    Ok::<u32, StreamError>(5_u32),
    Err::<u32, StreamError>(StreamError::Failed),
  ])
  .via(Flow::new().recover_with_retries(2, 9_u32))
  .collect_values()
  .expect("collect_values");
  assert_eq!(values, vec![9_u32, 5_u32, 9_u32]);
}

#[test]
fn recover_with_retries_fails_after_consuming_retry_budget() {
  let result =
    Source::from_array([Err::<u32, StreamError>(StreamError::Failed), Err::<u32, StreamError>(StreamError::Failed)])
      .via(Flow::new().recover_with_retries(1, 9_u32))
      .collect_values();
  assert_eq!(result, Err(StreamError::Failed));
}

#[test]
fn restart_flow_with_backoff_keeps_single_path_behavior() {
  let values =
    Source::single(7_u32).via(Flow::new().restart_flow_with_backoff(1, 3)).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn on_failures_with_backoff_alias_keeps_single_path_behavior() {
  let values =
    Source::single(7_u32).via(Flow::new().on_failures_with_backoff(1, 3)).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn with_backoff_alias_keeps_single_path_behavior() {
  let values = Source::single(7_u32).via(Flow::new().with_backoff(1, 3)).collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn with_backoff_and_context_alias_keeps_single_path_behavior() {
  let values = Source::single(7_u32)
    .via(Flow::new().with_backoff_and_context(1, 3, "compat"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn restart_flow_with_settings_keeps_single_path_behavior() {
  let settings = RestartSettings::new(1, 4, 3)
    .with_random_factor_permille(250)
    .with_max_restarts_within_ticks(16)
    .with_jitter_seed(17);
  let values = Source::single(7_u32)
    .via(Flow::new().restart_flow_with_settings(settings))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn supervision_variants_keep_single_path_behavior() {
  let values = Source::single(7_u32)
    .via(Flow::new().supervision_stop().supervision_resume().supervision_restart())
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn zip_logic_on_restart_clears_pending_state() {
  let mut logic = super::ZipLogic::<u32> { fan_in: 2, edge_slots: Vec::new(), pending: Vec::new() };

  let first = logic.apply_with_edge(0, Box::new(1_u32)).expect("first apply");
  assert!(first.is_empty());

  logic.on_restart().expect("restart");

  let second = logic.apply_with_edge(1, Box::new(2_u32)).expect("second apply");
  assert!(second.is_empty());
}

#[test]
fn concat_logic_on_restart_clears_pending_state() {
  let mut logic = super::ConcatLogic::<u32> {
    fan_in:      2,
    edge_slots:  Vec::new(),
    pending:     Vec::new(),
    active_slot: 0,
    source_done: false,
  };

  let from_left = logic.apply_with_edge(0, Box::new(1_u32)).expect("left apply");
  assert_eq!(from_left.len(), 1);
  let initial = logic.apply_with_edge(1, Box::new(9_u32)).expect("right apply");
  assert!(initial.is_empty());
  logic.on_source_done().expect("source done");

  logic.on_restart().expect("restart");

  let drained = logic.drain_pending().expect("drain");
  assert!(drained.is_empty());
}

#[test]
fn zip_with_index_logic_on_restart_resets_counter() {
  let mut logic = super::ZipWithIndexLogic::<u32> { next_index: 0, _pd: core::marker::PhantomData };
  let first = logic.apply(Box::new(10_u32)).expect("first apply");
  let second = logic.apply(Box::new(11_u32)).expect("second apply");
  assert_eq!(first.len(), 1);
  assert_eq!(second.len(), 1);

  logic.on_restart().expect("restart");

  let after_restart = logic.apply(Box::new(12_u32)).expect("after restart apply");
  assert_eq!(after_restart.len(), 1);
}

#[test]
fn stateful_map_logic_on_restart_recreates_mapper() {
  let factory = || {
    let mut sum = 0_u32;
    move |value: u32| {
      sum = sum.saturating_add(value);
      sum
    }
  };
  let mapper = factory();
  let mut logic = super::StatefulMapLogic::<u32, u32, _, _> { factory, mapper, _pd: core::marker::PhantomData };

  let first = logic.apply(Box::new(1_u32)).expect("first apply");
  let second = logic.apply(Box::new(2_u32)).expect("second apply");
  assert_eq!(first.len(), 1);
  assert_eq!(second.len(), 1);

  logic.on_restart().expect("restart");

  let third = logic.apply(Box::new(3_u32)).expect("third apply");
  assert_eq!(third.len(), 1);

  let third_value = *third.into_iter().next().expect("third value").downcast::<u32>().expect("u32");
  assert_eq!(third_value, 3_u32);
}

#[test]
fn stateful_map_concat_logic_on_restart_recreates_mapper() {
  let factory = || {
    let mut sum = 0_u32;
    move |value: u32| {
      sum = sum.saturating_add(value);
      [sum]
    }
  };
  let mapper = factory();
  let mut logic =
    super::StatefulMapConcatLogic::<u32, u32, _, _, [u32; 1]> { factory, mapper, _pd: core::marker::PhantomData };

  let first = logic.apply(Box::new(1_u32)).expect("first apply");
  let second = logic.apply(Box::new(2_u32)).expect("second apply");
  assert_eq!(first.len(), 1);
  assert_eq!(second.len(), 1);

  logic.on_restart().expect("restart");

  let third = logic.apply(Box::new(3_u32)).expect("third apply");
  let third_value = *third.into_iter().next().expect("third value").downcast::<u32>().expect("u32");
  assert_eq!(third_value, 3_u32);
}

#[test]
fn collect_type_collects_convertible_values() {
  let values = Source::from_array([1_i32, -1_i32, 2_i32])
    .via(Flow::new().collect_type::<u32>())
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32]);
}

#[test]
fn fold_async_emits_running_accumulation_when_future_is_ready() {
  let values = Source::from_array([1_u32, 2, 3])
    .via(Flow::new().fold_async(0_u32, |acc, value| async move { acc + value }))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 3_u32, 6_u32]);
}

#[test]
fn ask_alias_maps_values_asynchronously() {
  let flow = Flow::new().ask(1, |value: u32| async move { value + 1 }).expect("ask");
  let values = Source::from_array([1_u32, 2_u32]).via(flow).collect_values().expect("collect_values");
  assert_eq!(values, vec![2_u32, 3_u32]);
}

#[test]
fn ask_with_status_alias_maps_values_asynchronously() {
  let flow = Flow::new().ask_with_status(1, |value: u32| async move { value + 2 }).expect("ask_with_status");
  let values = Source::from_array([1_u32, 2_u32]).via(flow).collect_values().expect("collect_values");
  assert_eq!(values, vec![3_u32, 4_u32]);
}

#[test]
fn ask_with_context_preserves_context_and_maps_value() {
  let flow = Flow::<(u32, u32), (u32, u32), StreamNotUsed>::new()
    .ask_with_context(1, |value| async move { value + 10 })
    .expect("ask_with_context");
  let values = Source::from_array([(7_u32, 1_u32), (8_u32, 2_u32)]).via(flow).collect_values().expect("collect_values");
  assert_eq!(values, vec![(7_u32, 11_u32), (8_u32, 12_u32)]);
}

#[test]
fn ask_with_status_and_context_preserves_context_and_maps_value() {
  let flow = Flow::<(u32, u32), (u32, u32), StreamNotUsed>::new()
    .ask_with_status_and_context(1, |value| async move { value + 20 })
    .expect("ask_with_status_and_context");
  let values = Source::from_array([(7_u32, 1_u32), (8_u32, 2_u32)]).via(flow).collect_values().expect("collect_values");
  assert_eq!(values, vec![(7_u32, 21_u32), (8_u32, 22_u32)]);
}

#[test]
fn watch_alias_keeps_single_path_behavior() {
  let values = Source::from_array([1_u32, 2_u32]).via(Flow::new().watch()).collect_values().expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32]);
}

#[test]
fn operator_catalog_lookup_returns_contract_for_supported_operator() {
  let catalog = DefaultOperatorCatalog::new();
  let contract = catalog.lookup(OperatorKey::GROUP_BY).expect("lookup");
  assert_eq!(contract.key, OperatorKey::GROUP_BY);
  assert_eq!(contract.requirement_ids, &["1.1", "1.3", "2.1", "2.2"]);
}

#[test]
fn operator_catalog_lookup_returns_filter_contract() {
  let catalog = DefaultOperatorCatalog::new();
  let contract = catalog.lookup(OperatorKey::FILTER).expect("lookup");
  assert_eq!(contract.key, OperatorKey::FILTER);
  assert_eq!(contract.requirement_ids, &["1.1", "1.3"]);
}

#[test]
fn operator_catalog_lookup_returns_filter_not_contract() {
  let catalog = DefaultOperatorCatalog::new();
  let contract = catalog.lookup(OperatorKey::FILTER_NOT).expect("lookup");
  assert_eq!(contract.key, OperatorKey::FILTER_NOT);
  assert_eq!(contract.requirement_ids, &["1.1", "1.3"]);
}

#[test]
fn operator_catalog_lookup_returns_empty_contract() {
  let catalog = DefaultOperatorCatalog::new();
  let contract = catalog.lookup(OperatorKey::EMPTY).expect("lookup");
  assert_eq!(contract.key, OperatorKey::EMPTY);
  assert_eq!(contract.requirement_ids, &["1.1", "1.3"]);
}

#[test]
fn operator_catalog_lookup_returns_from_option_contract() {
  let catalog = DefaultOperatorCatalog::new();
  let contract = catalog.lookup(OperatorKey::FROM_OPTION).expect("lookup");
  assert_eq!(contract.key, OperatorKey::FROM_OPTION);
  assert_eq!(contract.requirement_ids, &["1.1", "1.3"]);
}

#[test]
fn operator_catalog_lookup_returns_from_array_contract() {
  let catalog = DefaultOperatorCatalog::new();
  let contract = catalog.lookup(OperatorKey::FROM_ARRAY).expect("lookup");
  assert_eq!(contract.key, OperatorKey::FROM_ARRAY);
  assert_eq!(contract.requirement_ids, &["1.1", "1.3"]);
}

#[test]
fn operator_catalog_lookup_returns_from_iterator_contract() {
  let catalog = DefaultOperatorCatalog::new();
  let contract = catalog.lookup(OperatorKey::FROM_ITERATOR).expect("lookup");
  assert_eq!(contract.key, OperatorKey::FROM_ITERATOR);
  assert_eq!(contract.requirement_ids, &["1.1", "1.3"]);
}

#[test]
fn operator_catalog_lookup_returns_flatten_optional_contract() {
  let catalog = DefaultOperatorCatalog::new();
  let contract = catalog.lookup(OperatorKey::FLATTEN_OPTIONAL).expect("lookup");
  assert_eq!(contract.key, OperatorKey::FLATTEN_OPTIONAL);
  assert_eq!(contract.requirement_ids, &["1.1", "1.3"]);
}

#[test]
fn operator_catalog_lookup_returns_stateful_map_contract() {
  let catalog = DefaultOperatorCatalog::new();
  let contract = catalog.lookup(OperatorKey::STATEFUL_MAP).expect("lookup");
  assert_eq!(contract.key, OperatorKey::STATEFUL_MAP);
  assert_eq!(contract.requirement_ids, &["1.1", "1.3"]);
}

#[test]
fn operator_catalog_lookup_returns_map_async_contract() {
  let catalog = DefaultOperatorCatalog::new();
  let contract = catalog.lookup(OperatorKey::MAP_ASYNC).expect("lookup");
  assert_eq!(contract.key, OperatorKey::MAP_ASYNC);
  assert_eq!(contract.requirement_ids, &["1.1", "1.3", "7.1", "7.2", "7.3", "7.4"]);
}

#[test]
fn operator_catalog_lookup_returns_stateful_map_concat_contract() {
  let catalog = DefaultOperatorCatalog::new();
  let contract = catalog.lookup(OperatorKey::STATEFUL_MAP_CONCAT).expect("lookup");
  assert_eq!(contract.key, OperatorKey::STATEFUL_MAP_CONCAT);
  assert_eq!(contract.requirement_ids, &["1.1", "1.3"]);
}

#[test]
fn operator_catalog_lookup_rejects_unknown_operator() {
  let catalog = DefaultOperatorCatalog::new();
  let result = catalog.lookup(OperatorKey::new("unsupported_operator"));
  assert_eq!(result, Err(StreamDslError::UnsupportedOperator { key: OperatorKey::new("unsupported_operator") }));
}

#[test]
fn detach_preserves_elements_and_order() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::new().detach())
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32]);
}

#[test]
fn log_passes_elements_through_unchanged() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::new().log("test"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32]);
}

#[test]
fn log_with_marker_passes_elements_through_unchanged() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::new().log_with_marker("test", "marker"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32]);
}

#[test]
fn do_on_first_invokes_callback_on_first_element_only() {
  use core::sync::atomic::{AtomicU32, Ordering};

  use fraktor_utils_rs::core::sync::ArcShared;

  let counter = ArcShared::new(AtomicU32::new(0));
  let counter_clone = counter.clone();
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[10, 20, 30]))
    .via(Flow::new().do_on_first(move |_value| {
      counter_clone.fetch_add(1, Ordering::Relaxed);
    }))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![10_u32, 20_u32, 30_u32]);
  assert_eq!(counter.load(Ordering::Relaxed), 1);
}

#[test]
fn conflate_preserves_elements_with_single_consumer() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::new().conflate())
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32]);
}

#[test]
fn fold_emits_running_accumulation_without_initial() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::new().fold(0_u32, |acc, value| acc + value))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 3_u32, 6_u32]);
}

#[test]
fn reduce_folds_with_first_element_as_seed() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::new().reduce(|acc, value| acc + value))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 3_u32, 6_u32]);
}

#[test]
fn reduce_single_element_emits_that_element() {
  let values =
    Source::single(42_u32).via(Flow::new().reduce(|acc, value| acc + value)).collect_values().expect("collect_values");
  assert_eq!(values, vec![42_u32]);
}

#[test]
fn fold_on_empty_stream_emits_nothing() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[]))
    .via(Flow::new().fold(0_u32, |acc, value| acc + value))
    .collect_values()
    .expect("collect_values");
  assert!(values.is_empty());
}

#[test]
fn reduce_on_empty_stream_emits_nothing() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[]))
    .via(Flow::new().reduce(|acc, value| acc + value))
    .collect_values()
    .expect("collect_values");
  assert!(values.is_empty());
}

#[test]
fn do_on_first_does_not_fire_on_empty_stream() {
  use core::sync::atomic::{AtomicU32, Ordering};

  use fraktor_utils_rs::core::sync::ArcShared;

  let counter = ArcShared::new(AtomicU32::new(0));
  let counter_clone = counter.clone();
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[]))
    .via(Flow::new().do_on_first(move |_value| {
      counter_clone.fetch_add(1, Ordering::Relaxed);
    }))
    .collect_values()
    .expect("collect_values");
  assert!(values.is_empty());
  assert_eq!(counter.load(Ordering::Relaxed), 0);
}

#[test]
fn from_function_transforms_elements() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::from_function(|x: u32| x + 1))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![2_u32, 3_u32, 4_u32]);
}

#[test]
fn named_passes_elements_through_unchanged() {
  let values = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic::new(&[1, 2, 3]))
    .via(Flow::new().named("test-stage"))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32]);
}

#[test]
fn also_to_mat_combines_materialized_values() {
  let (graph, (left_mat, right_mat)) =
    Flow::<u32, u32, StreamNotUsed>::new().also_to_mat(Sink::head(), KeepBoth).into_parts();
  let _ = graph;
  assert_eq!(left_mat, StreamNotUsed::new());
  assert_eq!(right_mat.poll(), Completion::Pending);
}

#[test]
fn also_to_mat_keeps_data_path_behavior() {
  let values = Source::single(1_u32)
    .via(Flow::new().map(|value: u32| value + 1).also_to_mat(Sink::ignore(), KeepBoth))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![2_u32]);
}

#[test]
fn wire_tap_mat_combines_materialized_values_and_keeps_data_path_behavior() {
  let (graph, materialized) = Flow::<u32, u32, StreamNotUsed>::new().wire_tap_mat(Sink::head(), KeepRight).into_parts();
  let _ = graph;
  assert_eq!(materialized.poll(), Completion::Pending);

  let values = Source::single(4_u32)
    .via(Flow::new().map(|value: u32| value + 1).wire_tap_mat(Sink::ignore(), KeepRight))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn monitor_mat_combines_materialized_values_and_keeps_data_path_behavior() {
  let (graph, _unused) = Flow::<u32, u32, StreamNotUsed>::new().into_parts();
  let flow: Flow<u32, u32, u32> = Flow::from_graph(graph, 21_u32);
  let (_graph, (left_mat, right_mat)) = flow.monitor_mat(KeepBoth).into_parts();
  assert_eq!(left_mat, 21_u32);
  assert_eq!(right_mat, FlowMonitor::<u32>::new());

  let values = Source::single(4_u32)
    .via(Flow::new().map(|value: u32| value + 1).monitor_mat(KeepLeft))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![5_u32]);
}

#[test]
fn flow_map_materialized_value_transforms_materialized_value_and_keeps_data_path_behavior() {
  let (graph, _unused) = Flow::<u32, u32, StreamNotUsed>::new().map(|value: u32| value + 1).into_parts();
  let flow: Flow<u32, u32, u32> = Flow::from_graph(graph, 10_u32);
  let (_graph, materialized) = flow.map_materialized_value(|value| value + 5).into_parts();
  assert_eq!(materialized, 15_u32);

  let values = Source::single(4_u32)
    .via(Flow::new().map(|value: u32| value + 1).map_materialized_value(|_| 1_u32))
    .collect_values()
    .expect("collect_values");
  assert_eq!(values, vec![5_u32]);
}
