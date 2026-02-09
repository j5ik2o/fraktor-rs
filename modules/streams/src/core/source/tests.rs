use fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox;

use super::super::{stream::Stream, stream_shared::StreamSharedGeneric};
use crate::core::{
  Materialized, Materializer, Sink, Source, StreamBufferConfig, StreamCompletion, StreamDone, StreamError,
  StreamHandleGeneric, StreamHandleId,
};

struct RecordingMaterializer {
  calls: usize,
}

impl RecordingMaterializer {
  const fn new() -> Self {
    Self { calls: 0 }
  }
}

impl Default for RecordingMaterializer {
  fn default() -> Self {
    Self::new()
  }
}

impl Materializer for RecordingMaterializer {
  type Toolbox = NoStdToolbox;

  fn start(&mut self) -> Result<(), StreamError> {
    Ok(())
  }

  fn materialize<Mat>(
    &mut self,
    graph: super::super::RunnableGraph<Mat>,
  ) -> Result<Materialized<Mat, Self::Toolbox>, StreamError> {
    self.calls += 1;
    let (plan, materialized) = graph.into_parts();
    let mut stream = Stream::new(plan, StreamBufferConfig::default());
    stream.start()?;
    let shared = StreamSharedGeneric::new(stream);
    let handle = StreamHandleGeneric::new(StreamHandleId::next(), shared);
    Ok(Materialized::new(handle, materialized))
  }

  fn shutdown(&mut self) -> Result<(), StreamError> {
    Ok(())
  }
}

#[test]
fn run_with_delegates_to_materializer_and_uses_sink_materialized_value() {
  let (graph, _completion) = Sink::<u32, StreamCompletion<StreamDone>>::ignore().into_parts();
  let marker = 7_u32;
  let sink = Sink::from_graph(graph, marker);
  let source = Source::single(1_u32);
  let mut materializer = RecordingMaterializer::default();
  let materialized = source.run_with(sink, &mut materializer).expect("run_with");
  assert_eq!(materializer.calls, 1);
  assert_eq!(*materialized.materialized(), marker);
}

#[test]
fn source_broadcast_duplicates_each_element() {
  let values = Source::single(5_u32).broadcast(2).collect_values().expect("collect_values");
  assert_eq!(values, vec![5_u32, 5_u32]);
}

#[test]
#[should_panic(expected = "fan_out must be greater than zero")]
fn source_broadcast_rejects_zero_fan_out() {
  let _ = Source::single(1_u32).broadcast(0);
}

#[test]
fn source_balance_keeps_single_path_behavior() {
  let values = Source::single(5_u32).balance(1).collect_values().expect("collect_values");
  assert_eq!(values, vec![5_u32]);
}

#[test]
#[should_panic(expected = "fan_out must be greater than zero")]
fn source_balance_rejects_zero_fan_out() {
  let _ = Source::single(1_u32).balance(0);
}

#[test]
fn source_merge_keeps_single_path_behavior() {
  let values = Source::single(5_u32).merge(1).collect_values().expect("collect_values");
  assert_eq!(values, vec![5_u32]);
}

#[test]
#[should_panic(expected = "fan_in must be greater than zero")]
fn source_merge_rejects_zero_fan_in() {
  let _ = Source::single(1_u32).merge(0);
}

#[test]
fn source_zip_wraps_value_when_single_path() {
  let values = Source::single(5_u32).zip(1).collect_values().expect("collect_values");
  assert_eq!(values, vec![vec![5_u32]]);
}

#[test]
#[should_panic(expected = "fan_in must be greater than zero")]
fn source_zip_rejects_zero_fan_in() {
  let _ = Source::single(1_u32).zip(0);
}

#[test]
fn source_concat_keeps_single_path_behavior() {
  let values = Source::single(5_u32).concat(1).collect_values().expect("collect_values");
  assert_eq!(values, vec![5_u32]);
}

#[test]
#[should_panic(expected = "fan_in must be greater than zero")]
fn source_concat_rejects_zero_fan_in() {
  let _ = Source::single(1_u32).concat(0);
}

#[test]
fn source_flat_map_merge_keeps_single_path_behavior() {
  let values = Source::single(5_u32).flat_map_merge(2, Source::single).collect_values().expect("collect_values");
  assert_eq!(values, vec![5_u32]);
}

#[test]
#[should_panic(expected = "breadth must be greater than zero")]
fn source_flat_map_merge_rejects_zero_breadth() {
  let _ = Source::single(1_u32).flat_map_merge(0, Source::single);
}
