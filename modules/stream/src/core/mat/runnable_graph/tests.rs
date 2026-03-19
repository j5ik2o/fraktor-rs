use super::super::lifecycle::{Stream, StreamShared};
use crate::core::{
  KeepLeft, KeepRight, StreamBufferConfig, StreamError,
  lifecycle::{SharedKillSwitch, StreamHandleId, StreamHandleImpl, StreamState},
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
  fn start(&mut self) -> Result<(), StreamError> {
    Ok(())
  }

  fn materialize<Mat>(&mut self, graph: RunnableGraph<Mat>) -> Result<Materialized<Mat>, StreamError> {
    self.calls += 1;
    let (plan, materialized) = graph.into_parts();
    let mut stream = Stream::new(plan, StreamBufferConfig::default());
    stream.start()?;
    let shared = StreamShared::new(stream);
    let handle = StreamHandleImpl::new(StreamHandleId::next(), shared);
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

#[test]
fn with_shared_kill_switch_allows_conflicting_flow_switch() {
  let flow_switch = SharedKillSwitch::new();
  let graph = Source::repeat(1_u32).via_mat(flow_switch.flow::<u32>(), KeepRight).to_mat(Sink::ignore(), KeepLeft);
  let external_switch = SharedKillSwitch::new();

  let graph = graph.with_shared_kill_switch(&external_switch);
  let (plan, materialized) = graph.into_parts();

  assert_eq!(plan.shared_kill_switch_states().len(), 1);

  let mut stream = Stream::new(plan, StreamBufferConfig::default());
  stream.start().expect("start");

  for _ in 0..3 {
    let _ = stream.drive();
  }
  assert_eq!(stream.state(), StreamState::Running);

  external_switch.shutdown();
  for _ in 0..4 {
    let _ = stream.drive();
    if stream.state().is_terminal() {
      break;
    }
  }

  assert_eq!(stream.state(), StreamState::Completed);
  assert!(!materialized.is_shutdown());
}
