use crate::core::{
  KeepRight,
  stage::{BidiFlow, Flow, Sink, Source},
};

#[test]
fn bidi_flow_split_returns_original_fragments() {
  let bidi = BidiFlow::from_flows(Flow::new().map(|value: u32| value + 1), Flow::new().map(|value: u32| value + 10));
  let (top, bottom) = bidi.split();

  let top_graph = Source::single(1_u32).via(top).to_mat(Sink::head(), KeepRight);
  let (top_plan, top_completion) = top_graph.into_parts();
  let mut top_interpreter =
    crate::core::graph::GraphInterpreter::new(top_plan, crate::core::StreamBufferConfig::default());
  top_interpreter.start().expect("start");
  while top_interpreter.state() == crate::core::lifecycle::StreamState::Running {
    let _ = top_interpreter.drive();
  }
  assert_eq!(top_completion.poll(), crate::core::Completion::Ready(Ok(2_u32)));

  let bottom_graph = Source::single(1_u32).via(bottom).to_mat(Sink::head(), KeepRight);
  let (bottom_plan, bottom_completion) = bottom_graph.into_parts();
  let mut bottom_interpreter =
    crate::core::graph::GraphInterpreter::new(bottom_plan, crate::core::StreamBufferConfig::default());
  bottom_interpreter.start().expect("start");
  while bottom_interpreter.state() == crate::core::lifecycle::StreamState::Running {
    let _ = bottom_interpreter.drive();
  }
  assert_eq!(bottom_completion.poll(), crate::core::Completion::Ready(Ok(11_u32)));
}

#[test]
fn bidi_flow_identity_passes_through_unchanged() {
  let bidi = BidiFlow::<u32, u32, u32, u32>::identity();
  let (top, bottom) = bidi.split();

  let top_graph = Source::single(42_u32).via(top).to_mat(Sink::head(), KeepRight);
  let (top_plan, top_completion) = top_graph.into_parts();
  let mut top_interpreter =
    crate::core::graph::GraphInterpreter::new(top_plan, crate::core::StreamBufferConfig::default());
  top_interpreter.start().expect("start");
  while top_interpreter.state() == crate::core::lifecycle::StreamState::Running {
    let _ = top_interpreter.drive();
  }
  assert_eq!(top_completion.poll(), crate::core::Completion::Ready(Ok(42_u32)));

  let bottom_graph = Source::single(99_u32).via(bottom).to_mat(Sink::head(), KeepRight);
  let (bottom_plan, bottom_completion) = bottom_graph.into_parts();
  let mut bottom_interpreter =
    crate::core::graph::GraphInterpreter::new(bottom_plan, crate::core::StreamBufferConfig::default());
  bottom_interpreter.start().expect("start");
  while bottom_interpreter.state() == crate::core::lifecycle::StreamState::Running {
    let _ = bottom_interpreter.drive();
  }
  assert_eq!(bottom_completion.poll(), crate::core::Completion::Ready(Ok(99_u32)));
}

#[test]
fn bidi_flow_reversed_swaps_top_and_bottom() {
  let bidi = BidiFlow::from_flows(Flow::new().map(|value: u32| value + 1), Flow::new().map(|value: u32| value + 10));
  let reversed = bidi.reversed();
  let (top, bottom) = reversed.split();

  // reversedのtopは元のbottom（value + 10）であること
  let top_graph = Source::single(1_u32).via(top).to_mat(Sink::head(), KeepRight);
  let (top_plan, top_completion) = top_graph.into_parts();
  let mut top_interpreter =
    crate::core::graph::GraphInterpreter::new(top_plan, crate::core::StreamBufferConfig::default());
  top_interpreter.start().expect("start");
  while top_interpreter.state() == crate::core::lifecycle::StreamState::Running {
    let _ = top_interpreter.drive();
  }
  assert_eq!(top_completion.poll(), crate::core::Completion::Ready(Ok(11_u32)));

  // reversedのbottomは元のtop（value + 1）であること
  let bottom_graph = Source::single(1_u32).via(bottom).to_mat(Sink::head(), KeepRight);
  let (bottom_plan, bottom_completion) = bottom_graph.into_parts();
  let mut bottom_interpreter =
    crate::core::graph::GraphInterpreter::new(bottom_plan, crate::core::StreamBufferConfig::default());
  bottom_interpreter.start().expect("start");
  while bottom_interpreter.state() == crate::core::lifecycle::StreamState::Running {
    let _ = bottom_interpreter.drive();
  }
  assert_eq!(bottom_completion.poll(), crate::core::Completion::Ready(Ok(2_u32)));
}
