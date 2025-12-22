use super::StreamGraph;
use crate::core::{flow::Flow, mat_combine::MatCombine, sink::Sink, source::Source};

#[test]
fn connect_tracks_connections() {
  let source = Source::<u32>::new();
  let flow = Flow::<u32, u32>::new();
  let sink = Sink::<u32>::new();

  let mut graph = StreamGraph::new();
  assert!(graph.connect(source.outlet(), flow.inlet(), MatCombine::KeepLeft).is_ok());
  assert!(graph.connect(flow.outlet(), sink.inlet(), MatCombine::KeepRight).is_ok());
  assert_eq!(graph.connection_count(), 2);

  let runnable = graph.build().expect("graph should build");
  assert_eq!(runnable.connection_count(), 2);
}
