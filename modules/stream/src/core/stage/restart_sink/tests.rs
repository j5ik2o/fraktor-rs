use super::RestartSink;
use crate::core::{Completion, KeepRight, RestartSettings, StreamDone};

#[test]
fn restart_sink_with_backoff_keeps_data_path_behavior() {
  let sink = RestartSink::with_backoff(crate::core::stage::Sink::<u32, _>::ignore(), 1, 3);
  let graph = crate::core::stage::Source::single(1_u32).to_mat(sink, KeepRight);
  let (plan, completion) = graph.into_parts();
  let mut interpreter = crate::core::graph::GraphInterpreter::new(plan, crate::core::StreamBufferConfig::default());
  interpreter.start().expect("start");
  while interpreter.state() == crate::core::lifecycle::StreamState::Running {
    let _ = interpreter.drive();
  }
  assert_eq!(completion.poll(), Completion::Ready(Ok(StreamDone::new())));
}

#[test]
fn restart_sink_with_settings_keeps_data_path_behavior() {
  let settings = RestartSettings::new(1, 2, 3);
  let sink = RestartSink::with_settings(crate::core::stage::Sink::<u32, _>::ignore(), settings);
  let graph = crate::core::stage::Source::single(2_u32).to_mat(sink, KeepRight);
  let (plan, completion) = graph.into_parts();
  let mut interpreter = crate::core::graph::GraphInterpreter::new(plan, crate::core::StreamBufferConfig::default());
  interpreter.start().expect("start");
  while interpreter.state() == crate::core::lifecycle::StreamState::Running {
    let _ = interpreter.drive();
  }
  assert_eq!(completion.poll(), Completion::Ready(Ok(StreamDone::new())));
}
