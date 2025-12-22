use std::time::Duration;

use crate::{
  core::{MatCombine, Materializer, Sink, Source, StreamGraph},
  std::tokio_materializer::TokioMaterializer,
};

#[tokio::test(flavor = "current_thread")]
async fn tokio_materializer_start_and_materialize() {
  let source = Source::<u32>::new();
  let sink = Sink::<u32>::new();
  let mut graph = StreamGraph::new();
  graph.connect(source.outlet(), sink.inlet(), MatCombine::KeepLeft).expect("connect");
  let runnable = graph.build().expect("build");

  let mut materializer = TokioMaterializer::new(Duration::from_millis(5));
  materializer.start().expect("start");

  let materialized = materializer.materialize(runnable).expect("materialize");
  let state = materialized.handle().state().expect("state");
  assert_eq!(state, crate::core::StreamState::Running);

  materializer.shutdown().expect("shutdown");
}
