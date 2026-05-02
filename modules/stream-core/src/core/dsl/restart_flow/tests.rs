use super::RestartFlow;
use crate::core::{
  RestartConfig,
  dsl::{Flow, Sink, Source, tests::RunWithCollectSink},
  r#impl::{
    fusing::StreamBufferConfig, interpreter::graph_interpreter::GraphInterpreter, materialization::StreamState,
  },
  materialization::{Completion, KeepRight},
};

#[test]
fn restart_flow_with_backoff_keeps_data_path_behavior() {
  let flow = RestartFlow::with_backoff(Flow::new().map(|value: u32| value + 1), 1, 3);
  let graph = Source::single(1_u32).via(flow).into_mat(Sink::head(), KeepRight);
  let (plan, completion) = graph.into_parts();
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  interpreter.start().expect("start");
  while interpreter.state() == StreamState::Running {
    let _ = interpreter.drive();
  }
  assert_eq!(completion.value(), Completion::Ready(Ok(2_u32)));
}

#[test]
fn restart_flow_with_settings_keeps_data_path_behavior() {
  let settings = RestartConfig::new(1, 2, 3);
  let flow = RestartFlow::with_settings(Flow::new().map(|value: u32| value + 1), settings);
  let values = Source::single(2_u32).via(flow).run_with_collect_sink().expect("run_with_collect_sink");
  assert_eq!(values, vec![3_u32]);
}
