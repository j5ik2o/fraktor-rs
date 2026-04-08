use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

use crate::{
  core::{
    StreamError,
    dsl::{Flow, Sink, Source},
    r#impl::{
      fusing::StreamBufferConfig, graph_dsl_builder::GraphDslBuilder, interpreter::GraphInterpreter,
      materialization::StreamState,
    },
    materialization::{Completion, DriveOutcome, StreamNotUsed},
  },
  graph_chain,
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

// --- graph_chain! macro: source => sink (no intermediate flows) ---

#[test]
fn graph_chain_source_to_sink_directly() -> Result<(), StreamError> {
  let observed = ArcShared::new(SpinSyncMutex::new(alloc::vec::Vec::new()));
  let observed_ref = observed.clone();
  let mut builder = GraphDslBuilder::<u32, u32, StreamNotUsed>::new();

  graph_chain!(builder; Source::single(42_u32) => Sink::<u32, _>::foreach(move |value| {
    observed_ref.lock().push(value);
  }));

  let (graph, _mat) = builder.into_parts();
  let plan = graph.into_plan()?;
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_terminal(&mut interpreter);
  assert_eq!(*observed.lock(), alloc::vec![42_u32]);
  Ok(())
}

// --- 手動配線: source => flow => sink (値検証) ---

#[test]
fn manual_wiring_source_flow_sink() -> Result<(), StreamError> {
  // Given: a builder
  let mut builder = GraphDslBuilder::<u32, u32, StreamNotUsed>::new();

  // 実行: source => flow を手動配線し、completion sink を接続する
  let source_out = builder.add_source(Source::single(5_u32))?;
  let out = builder.wire_via(&source_out, Flow::<u32, u32, StreamNotUsed>::new().map(|v| v * 2))?;
  let (sink_in, completion) = builder.add_sink_mat(Sink::head())?;
  builder.connect(&out, &sink_in)?;

  // Then: the graph produces 5 * 2 = 10
  let (graph, _mat) = builder.into_parts();
  let plan = graph.into_plan()?;
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_terminal(&mut interpreter);
  assert_eq!(completion.poll(), Completion::Ready(Ok(10_u32)));
  Ok(())
}

// --- graph_chain! macro: source => flow => flow => sink (verify value) ---

#[test]
fn manual_wiring_source_two_flows_sink() -> Result<(), StreamError> {
  // Given: a builder
  let mut builder = GraphDslBuilder::<u32, u32, StreamNotUsed>::new();

  // 実行: source => flow => flow を手動配線し、completion sink を接続する
  let source_out = builder.add_source(Source::single(3_u32))?;
  let out = builder.wire_via(&source_out, Flow::<u32, u32, StreamNotUsed>::new().map(|v| v + 1))?;
  let out = builder.wire_via(&out, Flow::<u32, u32, StreamNotUsed>::new().map(|v| v * 10))?;
  let (sink_in, completion) = builder.add_sink_mat(Sink::head())?;
  builder.connect(&out, &sink_in)?;

  // Then: the graph produces (3+1)*10 = 40
  let (graph, _mat) = builder.into_parts();
  let plan = graph.into_plan()?;
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_terminal(&mut interpreter);
  assert_eq!(completion.poll(), Completion::Ready(Ok(40_u32)));
  Ok(())
}

// --- 手動配線: source => flow => flow => flow => sink (値検証) ---

#[test]
fn manual_wiring_source_three_flows_sink() -> Result<(), StreamError> {
  // Given: a builder
  let mut builder = GraphDslBuilder::<u32, u32, StreamNotUsed>::new();

  // 実行: source => flow => flow => flow を手動配線し、completion sink を接続する
  let source_out = builder.add_source(Source::single(2_u32))?;
  let out = builder.wire_via(&source_out, Flow::<u32, u32, StreamNotUsed>::new().map(|v| v + 1))?;
  let out = builder.wire_via(&out, Flow::<u32, u32, StreamNotUsed>::new().map(|v| v * 2))?;
  let out = builder.wire_via(&out, Flow::<u32, u32, StreamNotUsed>::new().map(|v| v + 10))?;
  let (sink_in, completion) = builder.add_sink_mat(Sink::head())?;
  builder.connect(&out, &sink_in)?;

  // Then: the graph produces ((2+1)*2)+10 = 16
  let (graph, _mat) = builder.into_parts();
  let plan = graph.into_plan()?;
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_terminal(&mut interpreter);
  assert_eq!(completion.poll(), Completion::Ready(Ok(16_u32)));
  Ok(())
}

// --- graph_chain! macro: structural test with macro syntax ---

#[test]
fn graph_chain_macro_builds_graph() -> Result<(), StreamError> {
  let observed = ArcShared::new(SpinSyncMutex::new(alloc::vec::Vec::new()));
  let observed_ref = observed.clone();
  let mut builder = GraphDslBuilder::<u32, u32, StreamNotUsed>::new();

  graph_chain!(
    builder;
    Source::from_array([1_u32, 2, 3]) =>
    Flow::<u32, u32, StreamNotUsed>::new().map(|v| v * 10) =>
    Sink::<u32, _>::foreach(move |value| {
      observed_ref.lock().push(value);
    })
  );

  let (graph, _mat) = builder.into_parts();
  let plan = graph.into_plan()?;
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_terminal(&mut interpreter);
  assert_eq!(*observed.lock(), alloc::vec![10_u32, 20, 30]);
  Ok(())
}
