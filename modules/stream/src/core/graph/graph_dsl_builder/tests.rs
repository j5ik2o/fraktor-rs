use crate::core::{
  Completion, KeepBoth, KeepRight, StreamNotUsed,
  graph::{GraphDsl, GraphDslBuilder, GraphInterpreter},
  lifecycle::{DriveOutcome, StreamState},
  stage::{Sink, Source, flow::Flow},
};

fn drive_to_terminal(interpreter: &mut GraphInterpreter) {
  interpreter.start().expect("start");

  let mut idle_budget = 1024_usize;
  let mut drive_budget = 16384_usize;
  while interpreter.state() == StreamState::Running {
    assert!(drive_budget > 0, "stream did not reach terminal state within drive budget");
    drive_budget = drive_budget.saturating_sub(1);
    match interpreter.drive() {
      | DriveOutcome::Progressed => idle_budget = 1024,
      | DriveOutcome::Idle => {
        assert!(idle_budget > 0, "stream stalled");
        idle_budget = idle_budget.saturating_sub(1);
      },
    }
  }
}

#[test]
fn build_creates_flow_from_builder() {
  let flow = GraphDslBuilder::<u32, u32, StreamNotUsed>::new().via(Flow::new().map(|value| value + 1)).build();
  let graph = Source::single(1_u32).via(flow).to_mat(Sink::head(), KeepRight);
  let (plan, completion) = graph.into_parts();
  let mut interpreter = GraphInterpreter::new(plan, crate::core::StreamBufferConfig::default());

  drive_to_terminal(&mut interpreter);

  assert_eq!(completion.poll(), Completion::Ready(Ok(2_u32)));
}

#[test]
fn graph_dsl_facade_creates_builder() {
  let flow = GraphDsl::builder::<u32>().via(Flow::new().map(|value| value * 2)).build();
  let graph = Source::single(3_u32).via(flow).to_mat(Sink::head(), KeepRight);
  let (plan, completion) = graph.into_parts();
  let mut interpreter = GraphInterpreter::new(plan, crate::core::StreamBufferConfig::default());

  drive_to_terminal(&mut interpreter);

  assert_eq!(completion.poll(), Completion::Ready(Ok(6_u32)));
}

#[test]
fn graph_dsl_from_flow_maps_materialized_value() {
  let flow = GraphDsl::from_flow(Flow::<u32, u32, StreamNotUsed>::new().map(|value| value + 1))
    .map_materialized_value(|_| 7_u32)
    .via(Flow::new().map(|value| value * 2))
    .build();
  let graph = Source::single(2_u32).via_mat(flow, KeepRight).to_mat(Sink::head(), KeepBoth);
  let (plan, (mat, completion)) = graph.into_parts();
  let mut interpreter = GraphInterpreter::new(plan, crate::core::StreamBufferConfig::default());

  drive_to_terminal(&mut interpreter);

  assert_eq!(mat, 7_u32);
  assert_eq!(completion.poll(), Completion::Ready(Ok(6_u32)));
}

#[test]
fn to_mat_keeps_sink_materialized_value_rule() {
  let sink = GraphDslBuilder::<u32, u32, StreamNotUsed>::new()
    .via(Flow::new().map(|value| value + 3))
    .to_mat(Sink::head(), KeepRight);
  let graph = Source::single(4_u32).to_mat(sink, KeepRight);
  let (plan, completion) = graph.into_parts();
  let mut interpreter = GraphInterpreter::new(plan, crate::core::StreamBufferConfig::default());

  drive_to_terminal(&mut interpreter);

  assert_eq!(completion.poll(), Completion::Ready(Ok(7_u32)));
}

#[test]
fn from_flow_and_build_accept_non_sync_output_types() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new().map(Source::single);
  let _rebuilt: Flow<u32, Source<u32, StreamNotUsed>, StreamNotUsed> = GraphDslBuilder::from_flow(flow).build();
}
