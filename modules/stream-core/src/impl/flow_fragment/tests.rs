use crate::{
  dsl::{Flow, Sink, Source},
  r#impl::{
    flow_fragment::FlowFragment, fusing::StreamBufferConfig, interpreter::graph_interpreter::GraphInterpreter,
    materialization::StreamState,
  },
  materialization::{Completion, KeepRight},
};

#[test]
fn flow_fragment_builds_reusable_flow_fragment() {
  let fragment = FlowFragment::from_flow(Flow::new().map(|value: u32| value + 1));
  let flow = fragment.build();
  let graph = Source::single(1_u32).via(flow).into_mat(Sink::head(), KeepRight);
  let (_plan, completion) = graph.into_parts();
  assert_eq!(completion.value(), Completion::Pending);
}

#[test]
fn flow_fragment_via_and_to_compose_fragment() {
  let graph = Source::single(2_u32)
    .via(FlowFragment::from_flow(Flow::new().map(|value: u32| value + 1)).build())
    .into_mat(Sink::head(), KeepRight);
  let (plan, completion) = graph.into_parts();
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  interpreter.start().expect("start");
  while interpreter.state() == StreamState::Running {
    let _ = interpreter.drive();
  }
  assert_eq!(completion.value(), Completion::Ready(Ok(3_u32)));
}

#[test]
fn flow_fragment_supports_fan_out_and_fan_in() {
  let fragment = FlowFragment::from_flow(Flow::new().map(|value: u32| value + 1_u32))
    .broadcast(2)
    .expect("broadcast")
    .merge(2)
    .expect("merge");
  let _flow = fragment.build();
}

#[test]
fn flow_fragment_rejects_zero_fan_parameters() {
  assert!(FlowFragment::from_flow(Flow::new().map(|value: u32| value + 1)).broadcast(0).is_err());
  assert!(FlowFragment::from_flow(Flow::new().map(|value: u32| value + 1)).balance(0).is_err());
  assert!(FlowFragment::from_flow(Flow::new().map(|value: u32| value + 1)).merge(0).is_err());
  assert!(FlowFragment::from_flow(Flow::new().map(|value: u32| value + 1)).concat(0).is_err());
}
