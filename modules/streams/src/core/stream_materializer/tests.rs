use super::StreamMaterializer;
use crate::core::{
  flow::Flow, mat_combine::MatCombine, materializer::Materializer, sink::Sink, source::Source,
  stream_error::StreamError, stream_graph::StreamGraph,
};

#[test]
fn materializer_requires_start() {
  let source = Source::<u32>::new();
  let sink = Sink::<u32>::new();
  let mut graph = StreamGraph::new();
  graph.connect(source.outlet(), sink.inlet(), MatCombine::KeepLeft).expect("connect");
  let runnable = graph.build().expect("build");

  let mut materializer = StreamMaterializer::new();
  assert_eq!(materializer.materialize(runnable).err(), Some(StreamError::NotStarted));
}

#[test]
fn materializer_start_and_shutdown() {
  let mut materializer = StreamMaterializer::new();
  assert!(materializer.start().is_ok());
  assert_eq!(materializer.start(), Err(StreamError::AlreadyStarted));
  assert!(materializer.shutdown().is_ok());
}

#[test]
fn materializer_produces_handle() {
  let source = Source::<u32>::new();
  let flow = Flow::<u32, u32>::new();
  let sink = Sink::<u32>::new();

  let mut graph = StreamGraph::new();
  graph.connect(source.outlet(), flow.inlet(), MatCombine::KeepLeft).expect("connect");
  graph.connect(flow.outlet(), sink.inlet(), MatCombine::KeepRight).expect("connect");
  let runnable = graph.build().expect("build");

  let mut materializer = StreamMaterializer::new();
  materializer.start().expect("start");
  let materialized = materializer.materialize(runnable).expect("materialize");
  assert_eq!(materialized.value(), MatCombine::KeepRight);
}
