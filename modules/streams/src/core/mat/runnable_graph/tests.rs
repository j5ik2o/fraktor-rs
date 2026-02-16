use fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox;

use super::super::lifecycle::{Stream, StreamSharedGeneric};
use crate::core::{
  KeepRight, StreamBufferConfig, StreamError,
  lifecycle::{SharedKillSwitch, StreamHandleGeneric, StreamHandleId},
  mat::{Materialized, Materializer, RunnableGraph},
  stage::{Sink, Source},
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

#[test]
fn with_shared_kill_switch_keeps_materialized_value() {
  let marker = 321_u32;
  let (sink_graph, _completion) = Sink::<u32, _>::ignore().into_parts();
  let sink = Sink::<u32, u32>::from_graph(sink_graph, marker);
  let graph = Source::single(1_u32).to_mat(sink, KeepRight);
  let shared_kill_switch = SharedKillSwitch::new();

  let graph = graph.with_shared_kill_switch(&shared_kill_switch);

  assert_eq!(*graph.materialized(), marker);
}
