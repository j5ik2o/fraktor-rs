use crate::core::{Flow, MatCombine, Sink, Source};

#[test]
fn via_then_to_builds_graph_with_expected_state() {
  let source = Source::<u32>::new();
  let flow = Flow::<u32, u32>::new();
  let sink = Sink::<u32>::new();

  let runnable = source.via(&flow, MatCombine::KeepLeft).expect("via").to(&sink, MatCombine::KeepRight).expect("to");

  assert_eq!(runnable.connection_count(), 2);
  assert_eq!(runnable.materialized_value(), MatCombine::KeepRight);
}

#[test]
fn source_to_builds_single_connection_graph() {
  let source = Source::<u8>::new();
  let sink = Sink::<u8>::new();

  let runnable = source.to(&sink, MatCombine::KeepLeft).expect("to");

  assert_eq!(runnable.connection_count(), 1);
  assert_eq!(runnable.materialized_value(), MatCombine::KeepLeft);
}
