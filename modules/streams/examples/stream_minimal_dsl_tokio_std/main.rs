use std::time::Duration;

use fraktor_streams_rs::{
  core::{Flow, MatCombine, Materializer, Sink, Source},
  std::TokioMaterializer,
};

#[tokio::main(flavor = "current_thread")]
async fn main() {
  let source = Source::<u32>::new();
  let flow = Flow::<u32, u32>::new();
  let sink = Sink::<u32>::new();

  let runnable = source.via(&flow, MatCombine::KeepLeft).expect("via").to(&sink, MatCombine::KeepRight).expect("to");

  let mut materializer = TokioMaterializer::new(Duration::from_millis(10));
  materializer.start().expect("start");

  let materialized = materializer.materialize(runnable).expect("materialize");
  let state = materialized.handle().state().expect("state");
  println!("stream state: {:?}", state);

  materializer.shutdown().expect("shutdown");
}
