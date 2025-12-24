use fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox;

use super::super::{stream::Stream, stream_shared::StreamSharedGeneric};
use crate::core::{
  KeepRight, Materialized, Materializer, RunnableGraph, Sink, Source, StreamBufferConfig, StreamError,
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

  fn materialize<Mat>(&mut self, graph: RunnableGraph<Mat>) -> Result<Materialized<Mat, Self::Toolbox>, StreamError> {
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
fn run_delegates_to_materializer() {
  let graph = Source::single(1_u32).to_mat(Sink::head(), KeepRight);
  let mut materializer = RecordingMaterializer::default();
  let _materialized = graph.run(&mut materializer).expect("run");
  assert_eq!(materializer.calls, 1);
}
