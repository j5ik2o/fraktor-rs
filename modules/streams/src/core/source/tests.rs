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
