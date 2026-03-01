use super::RestartFlow;
use crate::core::{
  Completion, KeepRight, RestartSettings,
  stage::{Flow, Sink, Source},
};

#[test]
fn restart_flow_with_backoff_keeps_data_path_behavior() {
  let flow = RestartFlow::with_backoff(Flow::new().map(|value: u32| value + 1), 1, 3);
  let graph = Source::single(1_u32).via(flow).to_mat(Sink::head(), KeepRight);
  let (plan, completion) = graph.into_parts();
  let mut interpreter = crate::core::graph::GraphInterpreter::new(plan, crate::core::StreamBufferConfig::default());
  interpreter.start().expect("start");
  while interpreter.state() == crate::core::lifecycle::StreamState::Running {
    let _ = interpreter.drive();
  }
  assert_eq!(completion.poll(), Completion::Ready(Ok(2_u32)));
}

#[test]
fn restart_flow_with_settings_keeps_data_path_behavior() {
  let settings = RestartSettings::new(1, 2, 3);
  let flow = RestartFlow::with_settings(Flow::new().map(|value: u32| value + 1), settings);
  let values = Source::single(2_u32).via(flow).collect_values().expect("collect_values");
  assert_eq!(values, vec![3_u32]);
}
