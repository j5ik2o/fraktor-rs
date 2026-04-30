mod support;
use fraktor_stream_core_rs::core::{
  SubstreamCancelStrategy,
  dsl::{Flow, Source},
  materialization::StreamNotUsed,
};
use support::RunWithCollectSink;

#[test]
fn flow_split_when_accepts_drain_cancel_strategy() {
  let values = Source::single(7_u32)
    .via(
      Flow::<u32, u32, StreamNotUsed>::new()
        .split_when_with_cancel_strategy(SubstreamCancelStrategy::Drain, |_| false)
        .merge_substreams(),
    )
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn flow_split_after_accepts_propagate_cancel_strategy() {
  let values = Source::single(7_u32)
    .via(
      Flow::<u32, u32, StreamNotUsed>::new()
        .split_after_with_cancel_strategy(SubstreamCancelStrategy::Propagate, |_| false)
        .merge_substreams(),
    )
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn source_split_when_accepts_drain_cancel_strategy() {
  let values = Source::single(7_u32)
    .split_when_with_cancel_strategy(SubstreamCancelStrategy::Drain, |_| false)
    .merge_substreams()
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![7_u32]);
}

#[test]
fn source_split_after_accepts_propagate_cancel_strategy() {
  let values = Source::single(7_u32)
    .split_after_with_cancel_strategy(SubstreamCancelStrategy::Propagate, |_| false)
    .merge_substreams()
    .run_with_collect_sink()
    .expect("run_with_collect_sink");
  assert_eq!(values, vec![7_u32]);
}
