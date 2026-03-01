use crate::core::{
  KeepRight,
  graph::GraphDsl,
  stage::{Sink, Source},
};

#[test]
fn graph_dsl_builds_reusable_flow_fragment() {
  let dsl = GraphDsl::from_flow(crate::core::stage::Flow::new().map(|value: u32| value + 1));
  let flow = dsl.build();
  let graph = Source::single(1_u32).via(flow).to_mat(Sink::head(), KeepRight);
  let (_plan, completion) = graph.into_parts();
  assert_eq!(completion.poll(), crate::core::Completion::Pending);
}

#[test]
fn graph_dsl_via_and_to_compose_fragment() {
  let graph = Source::single(2_u32)
    .via(GraphDsl::from_flow(crate::core::stage::Flow::new().map(|value: u32| value + 1)).build())
    .to_mat(Sink::head(), KeepRight);
  let (plan, completion) = graph.into_parts();
  let mut interpreter = crate::core::graph::GraphInterpreter::new(plan, crate::core::StreamBufferConfig::default());
  interpreter.start().expect("start");
  while interpreter.state() == crate::core::lifecycle::StreamState::Running {
    let _ = interpreter.drive();
  }
  assert_eq!(completion.poll(), crate::core::Completion::Ready(Ok(3_u32)));
}

#[test]
fn graph_dsl_supports_fan_out_and_fan_in() {
  let dsl = GraphDsl::from_flow(crate::core::stage::Flow::new().map(|value: u32| value + 1_u32))
    .broadcast(2)
    .expect("broadcast")
    .merge(2)
    .expect("merge");
  let _flow = dsl.build();
}

#[test]
fn graph_dsl_rejects_zero_fan_parameters() {
  assert!(GraphDsl::from_flow(crate::core::stage::Flow::new().map(|value: u32| value + 1)).broadcast(0).is_err());
  assert!(GraphDsl::from_flow(crate::core::stage::Flow::new().map(|value: u32| value + 1)).balance(0).is_err());
  assert!(GraphDsl::from_flow(crate::core::stage::Flow::new().map(|value: u32| value + 1)).merge(0).is_err());
  assert!(GraphDsl::from_flow(crate::core::stage::Flow::new().map(|value: u32| value + 1)).concat(0).is_err());
}
