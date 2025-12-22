use std::time::Duration;

use fraktor_streams_rs::{
  core::{Flow, MatCombine, Materializer, Sink, Source, StreamGraph},
  std::TokioMaterializer,
};

#[tokio::main(flavor = "current_thread")]
async fn main() {
  let source = Source::<u32>::new();
  let flow = Flow::<u32, u32>::new();
  let sink = Sink::<u32>::new();

  let mut graph = StreamGraph::new();
  graph.connect(source.outlet(), flow.inlet(), MatCombine::KeepLeft).expect("connect");
  graph.connect(flow.outlet(), sink.inlet(), MatCombine::KeepRight).expect("connect");
  let runnable = graph.build().expect("build");

  let mut materializer = TokioMaterializer::new(Duration::from_millis(10));
  materializer.start().expect("start");

  let materialized = materializer.materialize(runnable).expect("materialize");
  let handle = materialized.into_handle();
  let state = handle.state().expect("state");
  println!("stream state: {:?}", state);

  materializer.shutdown().expect("shutdown");
}
